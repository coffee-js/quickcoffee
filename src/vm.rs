use crate::{
    ResourceLimit, ResourceLimits,
    bytecode::{Chunk, Constant, Instruction, Pattern},
    compile, json,
    lowering::{self, ChunkSourceMap, CompiledSourceMap},
    module::Module,
    parser,
};
use num_bigint::BigInt;
use num_integer::Integer as NumInteger;
use num_traits::{FromPrimitive, Signed, ToPrimitive, Zero};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_RANGE_ITEMS: i128 = 1_000_000;
const MAX_INTEGER_BITS: u64 = 1_000_000;
const MAX_DECIMAL_BITS: u64 = 1_000_000;
const MAX_DECIMAL_SCALE: u32 = 100_000;
const MAX_REUSABLE_CALL_ARGUMENTS: usize = 16;
const MAX_REUSABLE_FRAME_STACK: usize = 64;

/// Stable type tag for values crossing the embedding boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKind {
    /// The sole empty value.
    Nil,
    /// A strict boolean.
    Bool,
    /// An IEEE-754 number.
    Number,
    /// An arbitrary-precision signed integer.
    Integer,
    /// An exact normalized base-10 fixed-point value.
    Decimal,
    /// An immutable UTF-8 string.
    String,
    /// An immutable array.
    Array,
    /// An immutable string-keyed map.
    Map,
    /// A sealed structured script error.
    Error,
    /// An opaque QuickCoffee class.
    Class,
    /// An opaque QuickCoffee class instance.
    Instance,
    /// An opaque bytecode or native function.
    Function,
}

/// An opaque arbitrary-precision signed integer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Integer(BigInt);
impl Integer {
    /// Parses unsigned digits in the given radix, which must be from 2 through 36.
    pub fn parse_radix(digits: &str, radix: u32) -> Option<Self> {
        if !(2..=36).contains(&radix) {
            return None;
        }
        let value = BigInt::parse_bytes(digits.as_bytes(), radix)?;
        (value.bits() <= MAX_INTEGER_BITS).then_some(Self(value))
    }
    /// Returns the value when it fits in a signed 64-bit integer.
    pub fn as_i64(&self) -> Option<i64> {
        self.0.to_i64()
    }
    /// Returns the canonical base-10 representation.
    pub fn to_decimal_string(&self) -> String {
        self.0.to_string()
    }
    pub(crate) fn inner(&self) -> &BigInt {
        &self.0
    }
    pub(crate) fn from_bigint(value: BigInt) -> Result<Self, Error> {
        if value.bits() > MAX_INTEGER_BITS {
            Err(Error::runtime(
                "integer exceeds the implementation size limit",
            ))
        } else {
            Ok(Self(value))
        }
    }
}
impl From<i64> for Integer {
    fn from(value: i64) -> Self {
        Self(BigInt::from(value))
    }
}

/// An opaque exact normalized base-10 fixed-point value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decimal {
    coefficient: BigInt,
    scale: u32,
}
impl Decimal {
    /// Parses a signed decimal string with an optional fraction and base-10 exponent.
    pub fn parse(source: &str) -> Option<Self> {
        if source.is_empty() || source.trim() != source {
            return None;
        }
        let (negative, source) = match source.as_bytes().first() {
            Some(b'+') => (false, &source[1..]),
            Some(b'-') => (true, &source[1..]),
            _ => (false, source),
        };
        let mut exponent_parts = source.split(['e', 'E']);
        let mantissa = exponent_parts.next()?;
        let exponent = exponent_parts
            .next()
            .map_or(Some(0_i64), |value| value.parse().ok())?;
        if exponent_parts.next().is_some() || exponent.unsigned_abs() > u64::from(MAX_DECIMAL_SCALE)
        {
            return None;
        }
        let mut point_parts = mantissa.split('.');
        let whole = point_parts.next()?;
        let fraction = point_parts.next().unwrap_or("");
        if point_parts.next().is_some()
            || whole.is_empty()
            || !whole.bytes().all(|byte| byte.is_ascii_digit())
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let mut digits = String::with_capacity(whole.len() + fraction.len());
        digits.push_str(whole);
        digits.push_str(fraction);
        let mut scale = i64::try_from(fraction.len()).ok()?.checked_sub(exponent)?;
        if scale < 0 {
            let zeros = usize::try_from(-scale).ok()?;
            if zeros > MAX_DECIMAL_SCALE as usize {
                return None;
            }
            digits.extend(std::iter::repeat_n('0', zeros));
            scale = 0;
        }
        let scale = u32::try_from(scale).ok()?;
        if scale > MAX_DECIMAL_SCALE {
            return None;
        }
        let mut coefficient = BigInt::parse_bytes(digits.as_bytes(), 10)?;
        if negative {
            coefficient = -coefficient;
        }
        Self::from_bigint(coefficient, scale).ok()
    }
    /// Constructs a Decimal from a signed coefficient and a non-negative scale.
    pub fn from_parts(coefficient: Integer, scale: u32) -> Option<Self> {
        Self::from_bigint(coefficient.0, scale).ok()
    }
    /// Returns the normalized signed coefficient.
    pub fn coefficient(&self) -> Integer {
        Integer(self.coefficient.clone())
    }
    /// Returns the normalized number of fractional decimal digits.
    pub fn scale(&self) -> u32 {
        self.scale
    }
    /// Returns the canonical plain decimal representation without a type suffix.
    pub fn to_plain_string(&self) -> String {
        let negative = self.coefficient.is_negative();
        let digits = self.coefficient.abs().to_string();
        let mut output = String::new();
        if negative {
            output.push('-');
        }
        if self.scale == 0 {
            output.push_str(&digits);
        } else if digits.len() <= self.scale as usize {
            output.push_str("0.");
            output.extend(std::iter::repeat_n('0', self.scale as usize - digits.len()));
            output.push_str(&digits);
        } else {
            let point = digits.len() - self.scale as usize;
            output.push_str(&digits[..point]);
            output.push('.');
            output.push_str(&digits[point..]);
        }
        output
    }
    pub(crate) fn inner(&self) -> &BigInt {
        &self.coefficient
    }
    pub(crate) fn parse_with_resource_limits(
        source: &str,
        limits: ResourceLimits,
    ) -> Result<Self, Error> {
        decimal_text_resource_preflight(source, limits)?;
        let value = match Self::parse(source) {
            Some(value) => value,
            None if decimal_source_is_syntactically_valid(source) => {
                let limit = decimal_source_failure_limit(source);
                return Err(Error::resource(
                    limit,
                    "decimal text exceeds the implementation numeric limit",
                ));
            }
            None => return Err(Error::runtime("string is not a valid bounded decimal")),
        };
        check_decimal_resource(&value, limits)?;
        Ok(value)
    }
    pub(crate) fn from_bigint(mut coefficient: BigInt, mut scale: u32) -> Result<Self, Error> {
        if scale > MAX_DECIMAL_SCALE {
            return Err(Error::runtime(
                "decimal exceeds the implementation scale limit",
            ));
        }
        if coefficient.is_zero() {
            return Ok(Self {
                coefficient,
                scale: 0,
            });
        }
        if scale > 0 && (&coefficient % BigInt::from(10_u8)).is_zero() {
            let trailing_zeros = coefficient
                .to_string()
                .bytes()
                .rev()
                .take_while(|byte| *byte == b'0')
                .count();
            let removable = scale.min(trailing_zeros as u32);
            coefficient /= decimal_power_of_ten(removable);
            scale -= removable;
        }
        if coefficient.bits() > MAX_DECIMAL_BITS {
            Err(Error::runtime(
                "decimal exceeds the implementation size limit",
            ))
        } else {
            Ok(Self { coefficient, scale })
        }
    }
}
impl From<i64> for Decimal {
    fn from(value: i64) -> Self {
        Self {
            coefficient: BigInt::from(value),
            scale: 0,
        }
    }
}

pub(crate) fn decimal_source_is_syntactically_valid(source: &str) -> bool {
    if source.is_empty() || source.trim() != source {
        return false;
    }
    let source = source.strip_prefix(['+', '-']).unwrap_or(source);
    let mut exponent_parts = source.split(['e', 'E']);
    let Some(mantissa) = exponent_parts.next() else {
        return false;
    };
    if let Some(exponent) = exponent_parts.next() {
        let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        if exponent.is_empty() || !exponent.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
    }
    if exponent_parts.next().is_some() {
        return false;
    }
    let mut point_parts = mantissa.split('.');
    let whole = point_parts.next().unwrap_or("");
    let fraction = point_parts.next().unwrap_or("");
    point_parts.next().is_none()
        && !whole.is_empty()
        && whole.bytes().all(|byte| byte.is_ascii_digit())
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
}

pub(crate) fn decimal_source_failure_limit(source: &str) -> ResourceLimit {
    let source = source.strip_prefix(['+', '-']).unwrap_or(source);
    let mut exponent_parts = source.split(['e', 'E']);
    let mantissa = exponent_parts.next().unwrap_or("");
    let exponent = match exponent_parts.next().unwrap_or("0").parse::<i64>() {
        Ok(exponent) if exponent.unsigned_abs() <= u64::from(MAX_DECIMAL_SCALE) => exponent,
        _ => return ResourceLimit::DecimalScale,
    };
    let fraction_len = mantissa
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len());
    match i64::try_from(fraction_len)
        .ok()
        .and_then(|length| length.checked_sub(exponent))
    {
        Some(scale) if scale >= 0 && scale <= i64::from(MAX_DECIMAL_SCALE) => {
            ResourceLimit::DecimalCoefficientBits
        }
        Some(scale) if scale < 0 && scale.unsigned_abs() <= u64::from(MAX_DECIMAL_SCALE) => {
            ResourceLimit::DecimalCoefficientBits
        }
        _ => ResourceLimit::DecimalScale,
    }
}

pub(crate) fn decimal_text_resource_preflight(
    source: &str,
    limits: ResourceLimits,
) -> Result<(), Error> {
    if !decimal_source_is_syntactically_valid(source) {
        return Ok(());
    }
    let source = source.strip_prefix(['+', '-']).unwrap_or(source);
    let mut exponent_parts = source.split(['e', 'E']);
    let mantissa = exponent_parts.next().unwrap_or("");
    let exponent = exponent_parts.next().unwrap_or("0");
    let exponent = exponent.parse::<i64>().map_err(|_| {
        Error::resource(
            ResourceLimit::DecimalScale,
            "decimal exponent exceeds the implementation scale limit",
        )
    })?;
    if exponent.unsigned_abs() > u64::from(MAX_DECIMAL_SCALE) {
        return Err(Error::resource(
            ResourceLimit::DecimalScale,
            format!("decimal scale exceeds {}", decimal_scale_limit(limits)),
        ));
    }

    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let raw_scale = i64::try_from(fraction.len())
        .ok()
        .and_then(|length| length.checked_sub(exponent))
        .ok_or_else(|| {
            Error::resource(
                ResourceLimit::DecimalScale,
                "decimal scale exceeds the implementation limit",
            )
        })?;
    let appended_zeros = if raw_scale < 0 {
        usize::try_from(raw_scale.unsigned_abs()).map_err(|_| {
            Error::resource(
                ResourceLimit::DecimalScale,
                "decimal scale exceeds the implementation limit",
            )
        })?
    } else {
        0
    };
    if appended_zeros > MAX_DECIMAL_SCALE as usize {
        return Err(Error::resource(
            ResourceLimit::DecimalScale,
            "decimal exponent exceeds the implementation scale limit",
        ));
    }
    let raw_scale = u32::try_from(raw_scale.max(0)).map_err(|_| {
        Error::resource(
            ResourceLimit::DecimalScale,
            "decimal scale exceeds the implementation limit",
        )
    })?;
    let trailing_zeros = whole
        .bytes()
        .chain(fraction.bytes())
        .rev()
        .take_while(|byte| *byte == b'0')
        .count();
    let removable = raw_scale.min(u32::try_from(trailing_zeros).unwrap_or(u32::MAX));
    let normalized_scale = raw_scale - removable;
    let scale_limit = decimal_scale_limit(limits);
    if normalized_scale > scale_limit {
        return Err(Error::resource(
            ResourceLimit::DecimalScale,
            format!("decimal scale exceeds {scale_limit}"),
        ));
    }

    let original_digits = whole.len().saturating_add(fraction.len());
    let normalized_digits = original_digits
        .saturating_add(appended_zeros)
        .saturating_sub(removable as usize);
    let leading_zeros = whole
        .bytes()
        .chain(fraction.bytes())
        .take_while(|byte| *byte == b'0')
        .count()
        .min(normalized_digits);
    let significant_digits = if leading_zeros >= original_digits {
        0
    } else {
        normalized_digits.saturating_sub(leading_zeros)
    };
    let minimum_bits = if significant_digits == 0 {
        0
    } else {
        u64::try_from(significant_digits.saturating_sub(1))
            .unwrap_or(u64::MAX)
            .saturating_mul(3)
            .saturating_add(1)
    };
    let coefficient_limit = decimal_coefficient_bit_limit(limits);
    if minimum_bits > coefficient_limit {
        Err(Error::resource(
            ResourceLimit::DecimalCoefficientBits,
            format!("decimal coefficient magnitude exceeds {coefficient_limit} bits"),
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn integer_digits_resource_preflight(
    digits: &str,
    limits: ResourceLimits,
) -> Result<(), Error> {
    let significant_digits = digits.trim_start_matches('0').len();
    let minimum_bits = if significant_digits == 0 {
        0
    } else {
        u64::try_from(significant_digits.saturating_sub(1))
            .unwrap_or(u64::MAX)
            .saturating_mul(3)
            .saturating_add(1)
    };
    let limit = integer_bit_limit(limits);
    if minimum_bits > limit {
        Err(Error::resource(
            ResourceLimit::IntegerBits,
            format!("integer magnitude exceeds {limit} bits"),
        ))
    } else {
        Ok(())
    }
}

fn integer_bit_limit(limits: ResourceLimits) -> u64 {
    limits.max_integer_bits().min(MAX_INTEGER_BITS)
}

fn decimal_coefficient_bit_limit(limits: ResourceLimits) -> u64 {
    limits.max_decimal_coefficient_bits().min(MAX_DECIMAL_BITS)
}

fn decimal_scale_limit(limits: ResourceLimits) -> u32 {
    limits.max_decimal_scale().min(MAX_DECIMAL_SCALE)
}

fn check_integer_resource(value: &BigInt, limits: ResourceLimits) -> Result<(), Error> {
    let limit = integer_bit_limit(limits);
    if limit == MAX_INTEGER_BITS {
        return Ok(());
    }
    if value.bits() > limit {
        Err(Error::resource(
            ResourceLimit::IntegerBits,
            format!("integer magnitude exceeds {limit} bits"),
        ))
    } else {
        Ok(())
    }
}

fn check_decimal_resource(value: &Decimal, limits: ResourceLimits) -> Result<(), Error> {
    let scale_limit = decimal_scale_limit(limits);
    if scale_limit < MAX_DECIMAL_SCALE && value.scale > scale_limit {
        return Err(Error::resource(
            ResourceLimit::DecimalScale,
            format!("decimal scale exceeds {scale_limit}"),
        ));
    }
    let coefficient_limit = decimal_coefficient_bit_limit(limits);
    if coefficient_limit < MAX_DECIMAL_BITS && value.coefficient.bits() > coefficient_limit {
        Err(Error::resource(
            ResourceLimit::DecimalCoefficientBits,
            format!("decimal coefficient magnitude exceeds {coefficient_limit} bits"),
        ))
    } else {
        Ok(())
    }
}

#[inline]
fn resource_integer(value: BigInt, limits: ResourceLimits) -> Result<Integer, Error> {
    let limit = integer_bit_limit(limits);
    if value.bits() > limit {
        Err(Error::resource(
            ResourceLimit::IntegerBits,
            format!("integer magnitude exceeds {limit} bits"),
        ))
    } else {
        Ok(Integer(value))
    }
}

#[inline]
fn resource_decimal(
    coefficient: BigInt,
    scale: u32,
    limits: ResourceLimits,
) -> Result<Decimal, Error> {
    let value = Decimal::from_bigint(coefficient, scale).map_err(|_| {
        if scale > MAX_DECIMAL_SCALE {
            Error::resource(
                ResourceLimit::DecimalScale,
                format!("decimal scale exceeds {MAX_DECIMAL_SCALE} implementation limit"),
            )
        } else {
            Error::resource(
                ResourceLimit::DecimalCoefficientBits,
                format!(
                    "decimal coefficient magnitude exceeds {MAX_DECIMAL_BITS} implementation bits"
                ),
            )
        }
    })?;
    if decimal_limits_active(limits) {
        check_decimal_resource(&value, limits)?;
    }
    Ok(value)
}
impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let scale = self.scale.max(other.scale);
        let left = &self.coefficient * decimal_power_of_ten(scale - self.scale);
        let right = &other.coefficient * decimal_power_of_ten(scale - other.scale);
        left.cmp(&right)
    }
}
impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn decimal_power_of_ten(exponent: u32) -> BigInt {
    BigInt::from(10_u8).pow(exponent)
}

#[inline]
fn check_decimal_power_growth(
    coefficient: &BigInt,
    exponent: u32,
    limits: ResourceLimits,
) -> Result<(), Error> {
    let limit = decimal_coefficient_bit_limit(limits);
    if limit == MAX_DECIMAL_BITS || coefficient.is_zero() {
        return Ok(());
    }
    let minimum_bits = coefficient
        .bits()
        .saturating_add(u64::from(exponent).saturating_mul(3))
        .saturating_sub(1);
    if minimum_bits > limit {
        Err(Error::resource(
            ResourceLimit::DecimalCoefficientBits,
            format!("decimal coefficient magnitude exceeds {limit} bits"),
        ))
    } else {
        Ok(())
    }
}

#[inline]
fn decimal_add(left: &Decimal, right: &Decimal, limits: ResourceLimits) -> Result<Decimal, Error> {
    let scale = left.scale.max(right.scale);
    if decimal_limits_active(limits) {
        check_decimal_power_growth(left.inner(), scale - left.scale, limits)?;
        check_decimal_power_growth(right.inner(), scale - right.scale, limits)?;
    }
    resource_decimal(
        left.inner() * decimal_power_of_ten(scale - left.scale)
            + right.inner() * decimal_power_of_ten(scale - right.scale),
        scale,
        limits,
    )
}

#[inline]
fn decimal_sub(left: &Decimal, right: &Decimal, limits: ResourceLimits) -> Result<Decimal, Error> {
    let scale = left.scale.max(right.scale);
    if decimal_limits_active(limits) {
        check_decimal_power_growth(left.inner(), scale - left.scale, limits)?;
        check_decimal_power_growth(right.inner(), scale - right.scale, limits)?;
    }
    resource_decimal(
        left.inner() * decimal_power_of_ten(scale - left.scale)
            - right.inner() * decimal_power_of_ten(scale - right.scale),
        scale,
        limits,
    )
}

fn decimal_mul(left: &Decimal, right: &Decimal, limits: ResourceLimits) -> Result<Decimal, Error> {
    let scale = left
        .scale
        .checked_add(right.scale)
        .ok_or_else(|| Error::runtime("decimal multiplication exceeds the scale limit"))?;
    if scale > decimal_scale_limit(limits) {
        return Err(Error::resource(
            ResourceLimit::DecimalScale,
            format!("decimal scale exceeds {}", decimal_scale_limit(limits)),
        ));
    }
    let minimum_bits = left
        .inner()
        .bits()
        .saturating_add(right.inner().bits())
        .saturating_sub(1);
    if !left.inner().is_zero()
        && !right.inner().is_zero()
        && minimum_bits > decimal_coefficient_bit_limit(limits)
    {
        return Err(Error::resource(
            ResourceLimit::DecimalCoefficientBits,
            format!(
                "decimal coefficient magnitude exceeds {} bits",
                decimal_coefficient_bit_limit(limits)
            ),
        ));
    }
    resource_decimal(left.inner() * right.inner(), scale, limits)
}

fn bounded_factor_exponent(value: &BigInt, factor: u8, limit: u32) -> u32 {
    let factor = BigInt::from(factor);
    let mut low = 0_u32;
    let mut high = limit.saturating_add(1);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if (value % factor.pow(middle)).is_zero() {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low
}

fn decimal_exact_div(
    left: &Decimal,
    right: &Decimal,
    limits: ResourceLimits,
) -> Result<Decimal, Error> {
    if right.inner().is_zero() {
        return Err(Error::runtime("decimal division by zero"));
    }
    check_decimal_power_growth(left.inner(), right.scale, limits)?;
    check_decimal_power_growth(right.inner(), left.scale, limits)?;
    let mut numerator = left.inner() * decimal_power_of_ten(right.scale);
    let mut denominator = right.inner() * decimal_power_of_ten(left.scale);
    let gcd = numerator.gcd(&denominator);
    numerator /= &gcd;
    denominator /= gcd;
    if denominator.is_negative() {
        numerator = -numerator;
        denominator = -denominator;
    }
    let two = BigInt::from(2_u8);
    let five = BigInt::from(5_u8);
    let scale_limit = decimal_scale_limit(limits);
    let twos = bounded_factor_exponent(&denominator, 2, scale_limit);
    let fives = bounded_factor_exponent(&denominator, 5, scale_limit);
    if twos > scale_limit || fives > scale_limit {
        return Err(Error::resource(
            ResourceLimit::DecimalScale,
            format!("decimal scale exceeds {scale_limit}"),
        ));
    }
    denominator /= two.pow(twos) * five.pow(fives);
    if denominator != BigInt::from(1_u8) {
        return Err(Error::runtime(
            "decimal division is non-terminating; use decimal_div with an explicit scale and rounding mode",
        ));
    }
    let scale = twos.max(fives);
    check_decimal_power_growth(&numerator, scale - twos, limits)?;
    check_decimal_power_growth(&numerator, scale - fives, limits)?;
    numerator *= two.pow(scale - twos) * five.pow(scale - fives);
    resource_decimal(numerator, scale, limits)
}

fn aligned_decimal_coefficients(
    left: &Decimal,
    right: &Decimal,
    limits: ResourceLimits,
) -> Result<(BigInt, BigInt, u32), Error> {
    let scale = left.scale.max(right.scale);
    check_decimal_power_growth(left.inner(), scale - left.scale, limits)?;
    check_decimal_power_growth(right.inner(), scale - right.scale, limits)?;
    let left = left.inner() * decimal_power_of_ten(scale - left.scale);
    let right = right.inner() * decimal_power_of_ten(scale - right.scale);
    let limit = decimal_coefficient_bit_limit(limits);
    if limit < MAX_DECIMAL_BITS && (left.bits() > limit || right.bits() > limit) {
        return Err(Error::resource(
            ResourceLimit::DecimalCoefficientBits,
            format!("decimal aligned coefficient magnitude exceeds {limit} bits"),
        ));
    }
    Ok((left, right, scale))
}

fn decimal_cmp_resource(
    left: &Decimal,
    right: &Decimal,
    limits: ResourceLimits,
) -> Result<std::cmp::Ordering, Error> {
    let (left, right, _) = aligned_decimal_coefficients(left, right, limits)?;
    Ok(left.cmp(&right))
}

fn decimal_floor_div(
    left: &Decimal,
    right: &Decimal,
    limits: ResourceLimits,
) -> Result<Decimal, Error> {
    if right.inner().is_zero() {
        return Err(Error::runtime("decimal floor division by zero"));
    }
    let (left, right, _) = aligned_decimal_coefficients(left, right, limits)?;
    resource_decimal(integer_floor_div(&left, &right)?, 0, limits)
}

fn decimal_rem(left: &Decimal, right: &Decimal, limits: ResourceLimits) -> Result<Decimal, Error> {
    if right.inner().is_zero() {
        return Err(Error::runtime("decimal remainder by zero"));
    }
    let (left, right, scale) = aligned_decimal_coefficients(left, right, limits)?;
    resource_decimal(integer_rem(&left, &right)?, scale, limits)
}

fn decimal_modulo(
    left: &Decimal,
    right: &Decimal,
    limits: ResourceLimits,
) -> Result<Decimal, Error> {
    if right.inner().is_zero() {
        return Err(Error::runtime("decimal modulo by zero"));
    }
    let (left, right, scale) = aligned_decimal_coefficients(left, right, limits)?;
    resource_decimal(integer_modulo(&left, &right)?, scale, limits)
}

fn decimal_pow(left: &Decimal, right: &Decimal, limits: ResourceLimits) -> Result<Decimal, Error> {
    if right.scale != 0 {
        return Err(Error::runtime(
            "decimal exponent must be a non-negative whole Decimal",
        ));
    }
    let exponent = right.inner().to_u32().ok_or_else(|| {
        Error::runtime("decimal exponent must be a non-negative 32-bit whole Decimal")
    })?;
    let scale = left
        .scale
        .checked_mul(exponent)
        .ok_or_else(|| Error::runtime("decimal power exceeds the implementation scale limit"))?;
    if left.inner().bits().saturating_mul(u64::from(exponent))
        > decimal_coefficient_bit_limit(limits)
        || scale > decimal_scale_limit(limits)
    {
        return Err(if scale > decimal_scale_limit(limits) {
            Error::resource(
                ResourceLimit::DecimalScale,
                format!("decimal scale exceeds {}", decimal_scale_limit(limits)),
            )
        } else {
            Error::resource(
                ResourceLimit::DecimalCoefficientBits,
                format!(
                    "decimal coefficient magnitude exceeds {} bits",
                    decimal_coefficient_bit_limit(limits)
                ),
            )
        });
    }
    resource_decimal(left.inner().pow(exponent), scale, limits)
}

#[derive(Clone, Copy)]
enum DecimalRounding {
    Down,
    Up,
    Floor,
    Ceiling,
    HalfUp,
    HalfEven,
}
impl DecimalRounding {
    fn parse(value: &Value) -> Result<Self, Error> {
        let Value::String(value) = value else {
            return Err(Error::runtime("decimal rounding mode must be a string"));
        };
        match value.as_ref() {
            "down" => Ok(Self::Down),
            "up" => Ok(Self::Up),
            "floor" => Ok(Self::Floor),
            "ceiling" => Ok(Self::Ceiling),
            "half_up" => Ok(Self::HalfUp),
            "half_even" => Ok(Self::HalfEven),
            _ => Err(Error::runtime(
                "decimal rounding mode must be down, up, floor, ceiling, half_up, or half_even",
            )),
        }
    }
}

fn decimal_scale_argument(value: &Value, limits: ResourceLimits) -> Result<u32, Error> {
    let scale = match value {
        Value::Number(value)
            if value.is_finite()
                && value.fract() == 0.
                && *value >= 0.
                && *value <= f64::from(MAX_DECIMAL_SCALE) =>
        {
            *value as u32
        }
        Value::Integer(value) => value.inner().to_u32().ok_or_else(|| {
            Error::runtime("decimal scale must be a non-negative bounded integer")
        })?,
        _ => {
            return Err(Error::runtime(
                "decimal scale must be a non-negative integer",
            ));
        }
    };
    if scale > decimal_scale_limit(limits) {
        Err(Error::resource(
            ResourceLimit::DecimalScale,
            format!("decimal scale exceeds {}", decimal_scale_limit(limits)),
        ))
    } else if scale > MAX_DECIMAL_SCALE {
        Err(Error::runtime(
            "decimal scale exceeds the implementation limit",
        ))
    } else {
        Ok(scale)
    }
}

fn round_decimal_ratio(
    mut numerator: BigInt,
    mut denominator: BigInt,
    rounding: DecimalRounding,
) -> Result<BigInt, Error> {
    if denominator.is_zero() {
        return Err(Error::runtime("decimal division by zero"));
    }
    if denominator.is_negative() {
        numerator = -numerator;
        denominator = -denominator;
    }
    let (quotient, remainder) = numerator.div_rem(&denominator);
    if remainder.is_zero() {
        return Ok(quotient);
    }
    let direction = numerator.signum();
    let increment = match rounding {
        DecimalRounding::Down => false,
        DecimalRounding::Up => true,
        DecimalRounding::Floor => numerator.is_negative(),
        DecimalRounding::Ceiling => numerator.is_positive(),
        DecimalRounding::HalfUp | DecimalRounding::HalfEven => {
            match (remainder.abs() * BigInt::from(2_u8)).cmp(&denominator) {
                std::cmp::Ordering::Less => false,
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Equal => {
                    matches!(rounding, DecimalRounding::HalfUp) || quotient.is_odd()
                }
            }
        }
    };
    Ok(if increment {
        quotient + direction
    } else {
        quotient
    })
}

fn decimal_div_rounded(
    left: &Decimal,
    right: &Decimal,
    scale: u32,
    rounding: DecimalRounding,
    limits: ResourceLimits,
) -> Result<Decimal, Error> {
    let numerator_scale = right
        .scale
        .checked_add(scale)
        .ok_or_else(|| Error::runtime("decimal division exceeds the scale limit"))?;
    if scale > decimal_scale_limit(limits) {
        return Err(Error::resource(
            ResourceLimit::DecimalScale,
            format!("decimal scale exceeds {}", decimal_scale_limit(limits)),
        ));
    }
    check_decimal_power_growth(left.inner(), numerator_scale, limits)?;
    check_decimal_power_growth(right.inner(), left.scale, limits)?;
    let numerator = left.inner() * decimal_power_of_ten(numerator_scale);
    let denominator = right.inner() * decimal_power_of_ten(left.scale);
    resource_decimal(
        round_decimal_ratio(numerator, denominator, rounding)?,
        scale,
        limits,
    )
}

fn decimal_round(
    value: &Decimal,
    scale: u32,
    rounding: DecimalRounding,
    limits: ResourceLimits,
) -> Result<Decimal, Error> {
    decimal_div_rounded(value, &Decimal::from(1_i64), scale, rounding, limits)
}

/// An immutable structured error visible to QuickCoffee scripts and embedding hosts.
#[derive(Clone)]
pub struct ScriptError {
    code: Rc<str>,
    message: Rc<str>,
    data: Value,
    cause: Option<Rc<ScriptError>>,
    trusted_labels: Vec<DiagnosticLabel>,
}
impl ScriptError {
    /// Returns the stable machine-readable domain code.
    pub fn code(&self) -> &str {
        &self.code
    }
    /// Returns the display-independent human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }
    /// Returns immutable domain data.
    pub fn data(&self) -> &Value {
        &self.data
    }
    /// Returns the optional structured cause.
    pub fn cause(&self) -> Option<&ScriptError> {
        self.cause.as_deref()
    }
}
impl fmt::Debug for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScriptError")
            .field("code", &self.code)
            .field("message", &self.message)
            .field("data", &self.data)
            .field("cause", &self.cause)
            .finish()
    }
}

/// An immutable value crossing the QuickCoffee/host boundary.
#[derive(Clone)]
pub enum Value {
    /// The sole empty value.
    Nil,
    /// A strict boolean.
    Bool(bool),
    /// An IEEE-754 number used by the language.
    Number(f64),
    /// An arbitrary-precision signed integer.
    Integer(Rc<Integer>),
    /// An exact normalized base-10 fixed-point value.
    Decimal(Rc<Decimal>),
    /// An immutable UTF-8 string.
    String(Rc<str>),
    /// An immutable array of values.
    Array(Rc<Vec<Value>>),
    /// An immutable map with string keys.
    Map(Rc<BTreeMap<String, Value>>),
    /// A sealed structured error.
    Error(Rc<ScriptError>),
    /// An opaque QuickCoffee class value.
    Class(Rc<Class>),
    /// An opaque QuickCoffee class instance.
    Instance(Rc<Instance>),
    /// An opaque bytecode or native function.
    Function(Rc<Function>),
}

/// Converts one host value into an immutable QuickCoffee [`Value`] without
/// applying language-level coercion.
pub trait IntoValue {
    /// Consumes this host value and returns its exact QuickCoffee counterpart.
    fn into_value(self) -> Value;
}

/// Strictly converts an immutable QuickCoffee [`Value`] into an owned host value.
///
/// Implementations never coerce between `Number`, `Integer`, and `Decimal`.
/// Container implementations recursively convert every child and return a
/// stable runtime error that identifies the first failing path.
pub trait TryFromValue: Sized {
    /// Converts `value`, or returns a stable type-mismatch runtime error.
    fn try_from_value(value: &Value) -> Result<Self, Error>;
}
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => write!(f, "nil"),
            Self::Bool(x) => write!(f, "{x}"),
            Self::Number(x) => write!(f, "{x}"),
            Self::Integer(x) => write!(f, "{}n", x.to_decimal_string()),
            Self::Decimal(x) => write!(f, "{}m", x.to_plain_string()),
            Self::String(x) => write!(f, "{x:?}"),
            Self::Array(x) => f.debug_list().entries(x.iter()).finish(),
            Self::Map(x) => f.debug_map().entries(x.iter()).finish(),
            Self::Error(error) => write!(f, "error({}): {}", error.code, error.message),
            Self::Class(class) => write!(f, "<class {}>", class.name),
            Self::Instance(instance) => write!(f, "<{} instance>", instance.class.name),
            Self::Function(_) => write!(f, "<function>"),
        }
    }
}
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => write!(f, "nil"),
            Self::Bool(x) => write!(f, "{x}"),
            Self::Number(x) => write!(f, "{x}"),
            Self::Integer(x) => write!(f, "{}n", x.to_decimal_string()),
            Self::Decimal(x) => write!(f, "{}m", x.to_plain_string()),
            Self::String(x) => write!(f, "{x}"),
            Self::Array(x) => {
                write!(f, "[")?;
                for (i, v) in x.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?
                    }
                    write!(f, "{v}")?
                }
                write!(f, "]")
            }
            Self::Map(x) => {
                write!(f, "{{")?;
                for (i, (k, v)) in x.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?
                    }
                    write!(f, "{k}: {v}")?
                }
                write!(f, "}}")
            }
            Self::Error(error) => write!(f, "error({}): {}", error.code, error.message),
            Self::Class(class) => write!(f, "<class {}>", class.name),
            Self::Instance(instance) => write!(f, "<{} instance>", instance.class.name),
            Self::Function(_) => write!(f, "<function>"),
        }
    }
}
impl Value {
    /// Returns a stable type tag without exposing the internal container representation.
    pub fn kind(&self) -> ValueKind {
        match self {
            Self::Nil => ValueKind::Nil,
            Self::Bool(_) => ValueKind::Bool,
            Self::Number(_) => ValueKind::Number,
            Self::Integer(_) => ValueKind::Integer,
            Self::Decimal(_) => ValueKind::Decimal,
            Self::String(_) => ValueKind::String,
            Self::Array(_) => ValueKind::Array,
            Self::Map(_) => ValueKind::Map,
            Self::Error(_) => ValueKind::Error,
            Self::Class(_) => ValueKind::Class,
            Self::Instance(_) => ValueKind::Instance,
            Self::Function(_) => ValueKind::Function,
        }
    }
    /// Returns whether this value is the language's `nil` value.
    pub fn is_nil(&self) -> bool {
        matches!(self, Self::Nil)
    }
    /// Builds a QuickCoffee string without exposing its `Rc<str>` storage.
    pub fn string(value: impl Into<Rc<str>>) -> Self {
        Self::String(value.into())
    }
    /// Builds an immutable QuickCoffee array from host values.
    pub fn array(values: impl Into<Vec<Value>>) -> Self {
        Self::Array(Rc::new(values.into()))
    }
    /// Builds an immutable QuickCoffee map from host key/value entries.
    pub fn map<I, K>(entries: I) -> Self
    where
        I: IntoIterator<Item = (K, Value)>,
        K: Into<String>,
    {
        Self::Map(Rc::new(
            entries
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        ))
    }
    /// Returns the number, if this value is numeric.
    pub fn as_number(&self) -> Option<f64> {
        if let Self::Number(x) = self {
            Some(*x)
        } else {
            None
        }
    }
    /// Returns the exact integer, if this value is an integer.
    pub fn as_integer(&self) -> Option<&Integer> {
        if let Self::Integer(value) = self {
            Some(value)
        } else {
            None
        }
    }
    /// Returns the exact Decimal, if this value is a Decimal.
    pub fn as_decimal(&self) -> Option<&Decimal> {
        if let Self::Decimal(value) = self {
            Some(value)
        } else {
            None
        }
    }
    /// Builds an exact integer from a signed 64-bit host value.
    pub fn integer(value: impl Into<Integer>) -> Self {
        Self::Integer(Rc::new(value.into()))
    }
    /// Returns the boolean, if this value is boolean.
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(x) = self {
            Some(*x)
        } else {
            None
        }
    }
    /// Returns the UTF-8 view, if this value is a string.
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(x) = self {
            Some(x)
        } else {
            None
        }
    }
    /// Returns an immutable slice, if this value is an array.
    pub fn as_array(&self) -> Option<&[Value]> {
        if let Self::Array(values) = self {
            Some(values)
        } else {
            None
        }
    }
    /// Returns an immutable map view, if this value is a map.
    pub fn as_map(&self) -> Option<&BTreeMap<String, Value>> {
        if let Self::Map(values) = self {
            Some(values)
        } else {
            None
        }
    }
    /// Returns the structured error, if this value is an Error.
    pub fn as_error(&self) -> Option<&ScriptError> {
        if let Self::Error(error) = self {
            Some(error)
        } else {
            None
        }
    }
}
impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}
impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}
impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::integer(value)
    }
}
impl From<Decimal> for Value {
    fn from(value: Decimal) -> Self {
        Self::Decimal(Rc::new(value))
    }
}
impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::string(value)
    }
}
impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::string(value)
    }
}

impl IntoValue for Value {
    fn into_value(self) -> Value {
        self
    }
}
impl IntoValue for () {
    fn into_value(self) -> Value {
        Value::Nil
    }
}
impl IntoValue for bool {
    fn into_value(self) -> Value {
        Value::Bool(self)
    }
}
impl IntoValue for f64 {
    fn into_value(self) -> Value {
        Value::Number(self)
    }
}
impl IntoValue for Integer {
    fn into_value(self) -> Value {
        Value::Integer(Rc::new(self))
    }
}
impl IntoValue for Decimal {
    fn into_value(self) -> Value {
        Value::Decimal(Rc::new(self))
    }
}
impl IntoValue for String {
    fn into_value(self) -> Value {
        Value::string(self)
    }
}
impl IntoValue for &str {
    fn into_value(self) -> Value {
        Value::string(self)
    }
}
impl<T> IntoValue for Vec<T>
where
    T: IntoValue,
{
    fn into_value(self) -> Value {
        Value::array(
            self.into_iter()
                .map(IntoValue::into_value)
                .collect::<Vec<_>>(),
        )
    }
}
impl<T> IntoValue for BTreeMap<String, T>
where
    T: IntoValue,
{
    fn into_value(self) -> Value {
        Value::map(
            self.into_iter()
                .map(|(key, value)| (key, value.into_value())),
        )
    }
}
impl<T> IntoValue for Option<T>
where
    T: IntoValue,
{
    fn into_value(self) -> Value {
        self.map_or(Value::Nil, IntoValue::into_value)
    }
}

impl TryFromValue for Value {
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        Ok(value.clone())
    }
}
impl TryFromValue for () {
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        matches!(value, Value::Nil)
            .then_some(())
            .ok_or_else(|| value_type_error("nil", value))
    }
}
impl TryFromValue for bool {
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        value
            .as_bool()
            .ok_or_else(|| value_type_error("bool", value))
    }
}
impl TryFromValue for f64 {
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        value
            .as_number()
            .ok_or_else(|| value_type_error("number", value))
    }
}
impl TryFromValue for Integer {
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        value
            .as_integer()
            .cloned()
            .ok_or_else(|| value_type_error("integer", value))
    }
}
impl TryFromValue for Decimal {
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        value
            .as_decimal()
            .cloned()
            .ok_or_else(|| value_type_error("decimal", value))
    }
}
impl TryFromValue for String {
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| value_type_error("string", value))
    }
}
impl<T> TryFromValue for Vec<T>
where
    T: TryFromValue,
{
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        let values = value
            .as_array()
            .ok_or_else(|| value_type_error("array", value))?;
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                T::try_from_value(value)
                    .map_err(|error| conversion_path_error(format!("[{index}]"), error))
            })
            .collect()
    }
}
impl<T> TryFromValue for BTreeMap<String, T>
where
    T: TryFromValue,
{
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        let values = value
            .as_map()
            .ok_or_else(|| value_type_error("map", value))?;
        values
            .iter()
            .map(|(key, value)| {
                T::try_from_value(value)
                    .map(|value| (key.clone(), value))
                    .map_err(|error| conversion_path_error(format!(".{key}"), error))
            })
            .collect()
    }
}
impl<T> TryFromValue for Option<T>
where
    T: TryFromValue,
{
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        if value.is_nil() {
            Ok(None)
        } else {
            T::try_from_value(value).map(Some)
        }
    }
}

fn value_type_error(expected: &str, value: &Value) -> Error {
    Error::runtime(format!(
        "expected {expected}, got {}",
        value_kind_name(value)
    ))
}

fn conversion_path_error(path: String, error: Error) -> Error {
    Error::runtime(format!("value at {path}: {}", error.message()))
}

fn value_kind_name(value: &Value) -> &'static str {
    match value.kind() {
        ValueKind::Nil => "nil",
        ValueKind::Bool => "bool",
        ValueKind::Number => "number",
        ValueKind::Integer => "integer",
        ValueKind::Decimal => "decimal",
        ValueKind::String => "string",
        ValueKind::Array => "array",
        ValueKind::Map => "map",
        ValueKind::Error => "error",
        ValueKind::Class => "class",
        ValueKind::Instance => "instance",
        ValueKind::Function => "function",
    }
}

fn value_limits_active(_: ResourceLimits) -> bool {
    // General String/Array/Map defaults are real Context boundaries, unlike the
    // numeric absolute implementation ceilings. Loading an existing value must
    // therefore always recheck the current Context policy.
    true
}

#[inline]
fn value_needs_resource_check(value: &Value) -> bool {
    !matches!(value, Value::Nil | Value::Bool(_) | Value::Number(_))
}

#[inline(never)]
fn check_member_value_resources(value: &Value, limits: ResourceLimits) -> Result<(), Error> {
    if value_needs_resource_check(value) {
        check_value_resources(value, limits)?;
    }
    Ok(())
}

fn decimal_limits_active(limits: ResourceLimits) -> bool {
    decimal_coefficient_bit_limit(limits) < MAX_DECIMAL_BITS
        || decimal_scale_limit(limits) < MAX_DECIMAL_SCALE
}

fn check_string_resource(value: &str, limits: ResourceLimits) -> Result<(), Error> {
    check_string_len_resource(value.len(), limits)
}

fn check_string_len_resource(len: usize, limits: ResourceLimits) -> Result<(), Error> {
    if len > limits.max_string_bytes() {
        return Err(Error::resource(
            ResourceLimit::StringBytes,
            format!("string exceeds {} bytes", limits.max_string_bytes()),
        ));
    }
    Ok(())
}

fn check_array_resource(len: usize, limits: ResourceLimits) -> Result<(), Error> {
    if len > limits.max_array_items() {
        return Err(Error::resource(
            ResourceLimit::ArrayItems,
            format!("array exceeds {} items", limits.max_array_items()),
        ));
    }
    Ok(())
}

fn check_map_resource(len: usize, limits: ResourceLimits) -> Result<(), Error> {
    if len > limits.max_map_entries() {
        return Err(Error::resource(
            ResourceLimit::MapEntries,
            format!("map exceeds {} entries", limits.max_map_entries()),
        ));
    }
    Ok(())
}

fn check_value_resources(value: &Value, limits: ResourceLimits) -> Result<(), Error> {
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            Value::Integer(value) => check_integer_resource(value.inner(), limits)?,
            Value::Decimal(value) => check_decimal_resource(value, limits)?,
            Value::String(value) => check_string_resource(value, limits)?,
            Value::Array(values) => {
                check_array_resource(values.len(), limits)?;
                pending.extend(values.iter());
            }
            Value::Map(values) => {
                check_map_resource(values.len(), limits)?;
                for (key, value) in values.iter() {
                    check_string_resource(key, limits)?;
                    pending.push(value);
                }
            }
            Value::Error(error) => {
                check_string_resource(&error.code, limits)?;
                check_string_resource(&error.message, limits)?;
                pending.push(&error.data);
                let mut cause = error.cause.as_deref();
                while let Some(error) = cause {
                    check_string_resource(&error.code, limits)?;
                    check_string_resource(&error.message, limits)?;
                    pending.push(&error.data);
                    cause = error.cause.as_deref();
                }
            }
            Value::Nil
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Class(_)
            | Value::Instance(_)
            | Value::Function(_) => {}
        }
    }
    Ok(())
}

/// Stable category for an error crossing the Rust embedding boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Lexing or parsing failed.
    Parse,
    /// Untrusted bytecode failed verification.
    Verify,
    /// Execution or a host callback failed.
    Runtime,
    /// Execution stopped because a configured resource boundary was reached.
    Resource,
}
/// A source coordinate attached to a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    /// One-based line number.
    pub line: usize,
    /// One-based Unicode scalar column, or `None` when only the line is known.
    pub column: Option<usize>,
}
/// A half-open source range, or a line-only location when [`Self::end`] is absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    /// Opaque source name supplied by a CLI, module loader, or embedding host.
    pub source_name: Option<String>,
    /// Inclusive start coordinate.
    pub start: SourcePosition,
    /// Exclusive end coordinate, or `None` when the diagnostic is line-only.
    pub end: Option<SourcePosition>,
}
/// Role of a source label in a structured diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLabelKind {
    /// The source range that caused the error.
    Primary,
    /// A related source range that provides additional context.
    Secondary,
}
/// A display-independent source annotation attached to an [`Error`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticLabel {
    /// Whether this is the primary cause or related context.
    pub kind: DiagnosticLabelKind,
    /// Source range associated with this label.
    pub span: SourceSpan,
    /// Optional detail specific to this range, separate from [`Error::message`].
    pub message: Option<String>,
}
impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse => write!(f, "parse"),
            Self::Verify => write!(f, "verify"),
            Self::Runtime => write!(f, "runtime"),
            Self::Resource => write!(f, "resource"),
        }
    }
}
/// A structured error suitable for CLI display or host-side branching.
#[derive(Clone)]
pub struct Error {
    kind: ErrorKind,
    message: String,
    labels: Vec<DiagnosticLabel>,
    resource_limit: Option<ResourceLimit>,
    script_error: Option<Rc<ScriptError>>,
    verification_site: Option<VerificationSite>,
}
#[derive(Debug, Clone, Copy)]
struct VerificationSite {
    chunk: Option<usize>,
    instruction: usize,
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("kind", &self.kind)
            .field("message", &self.message)
            .field("labels", &self.labels)
            .field("resource_limit", &self.resource_limit)
            .field("script_error", &self.script_error)
            .finish()
    }
}
impl Error {
    pub(crate) fn parse(m: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Parse,
            message: m.into(),
            labels: Vec::new(),
            resource_limit: None,
            script_error: None,
            verification_site: None,
        }
    }
    pub(crate) fn verify(m: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Verify,
            message: m.into(),
            labels: Vec::new(),
            resource_limit: None,
            script_error: None,
            verification_site: None,
        }
    }
    /// Creates a runtime error for a host callback to return across the VM boundary.
    pub fn runtime(m: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Runtime,
            message: m.into(),
            labels: Vec::new(),
            resource_limit: None,
            script_error: None,
            verification_site: None,
        }
    }
    /// Creates a catchable domain error with machine-readable script fields.
    ///
    /// Invalid codes or non-business data deterministically become a generic
    /// runtime error rather than exposing an invalid script Error value.
    pub fn domain(code: impl Into<String>, message: impl Into<String>, data: Value) -> Self {
        let code = code.into();
        let message = message.into();
        if !valid_error_code(&code) || !valid_error_data(&data, 0) {
            return Self::runtime("host supplied invalid domain error fields");
        }
        let script_error = Rc::new(ScriptError {
            code: Rc::from(code.as_str()),
            message: Rc::from(message.as_str()),
            data,
            cause: None,
            trusted_labels: Vec::new(),
        });
        Self {
            kind: ErrorKind::Runtime,
            message,
            labels: Vec::new(),
            resource_limit: None,
            script_error: Some(script_error),
            verification_site: None,
        }
    }
    fn resource(limit: ResourceLimit, message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Resource,
            message: message.into(),
            labels: Vec::new(),
            resource_limit: Some(limit),
            script_error: None,
            verification_site: None,
        }
    }
    /// Returns the machine-readable category without requiring display-text parsing.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
    /// Returns the human-readable detail without its category prefix.
    pub fn message(&self) -> &str {
        &self.message
    }
    /// Returns the primary label's start coordinate when the compiler knows it.
    ///
    /// This compatibility accessor returns the same line as before structured
    /// labels were introduced. Call [`Self::labels`] to inspect complete ranges.
    pub fn position(&self) -> Option<SourcePosition> {
        self.labels
            .iter()
            .find(|label| label.kind == DiagnosticLabelKind::Primary)
            .map(|label| label.span.start)
    }
    /// Returns ordered, display-independent source annotations.
    pub fn labels(&self) -> &[DiagnosticLabel] {
        &self.labels
    }
    /// Returns the crossed resource boundary for a resource error.
    pub fn resource_limit(&self) -> Option<ResourceLimit> {
        self.resource_limit
    }
    /// Returns the structured script/domain error when this Runtime error carries one.
    pub fn script_error(&self) -> Option<&ScriptError> {
        self.script_error.as_deref()
    }
    fn from_script_error(script_error: Rc<ScriptError>) -> Self {
        Self {
            kind: ErrorKind::Runtime,
            message: script_error.message.to_string(),
            labels: script_error.trusted_labels.clone(),
            resource_limit: None,
            script_error: Some(script_error),
            verification_site: None,
        }
    }
    fn catch_value(&self) -> Value {
        if let Some(script_error) = &self.script_error {
            let mut caught = (**script_error).clone();
            caught.trusted_labels = self.labels.clone();
            return Value::Error(Rc::new(caught));
        }
        Value::Error(Rc::new(ScriptError {
            code: Rc::from("runtime"),
            message: Rc::from(self.message.as_str()),
            data: Value::Nil,
            cause: None,
            trusted_labels: self.labels.clone(),
        }))
    }
    pub(crate) fn at_line(mut self, line: usize) -> Self {
        self.labels = vec![DiagnosticLabel {
            kind: DiagnosticLabelKind::Primary,
            span: SourceSpan {
                source_name: None,
                start: SourcePosition { line, column: None },
                end: None,
            },
            message: None,
        }];
        self
    }
    pub(crate) fn at_span(mut self, span: SourceSpan) -> Self {
        self.labels = vec![DiagnosticLabel {
            kind: DiagnosticLabelKind::Primary,
            span,
            message: None,
        }];
        self
    }
    pub(crate) fn with_source_name(mut self, source_name: &str) -> Self {
        for label in &mut self.labels {
            if label.span.source_name.is_none() {
                label.span.source_name = Some(source_name.to_owned());
            }
        }
        self
    }
    pub(crate) fn with_span_if_missing(mut self, span: Option<SourceSpan>) -> Self {
        if self.labels.is_empty() {
            if let Some(span) = span {
                self = self.at_span(span);
            }
        }
        self
    }
    pub(crate) fn with_secondary_span(mut self, span: Option<SourceSpan>) -> Self {
        if let Some(span) = span {
            self.labels.push(DiagnosticLabel {
                kind: DiagnosticLabelKind::Secondary,
                span,
                message: Some("called from here".to_owned()),
            });
        }
        self
    }
    pub(crate) fn at_instruction(mut self, instruction: usize) -> Self {
        if self.verification_site.is_none() {
            self.verification_site = Some(VerificationSite {
                chunk: None,
                instruction,
            });
        }
        self
    }
    pub(crate) fn with_verification_chunk(mut self, chunk: usize) -> Self {
        if let Some(site) = &mut self.verification_site {
            if site.chunk.is_none() {
                site.chunk = Some(chunk);
            }
        }
        self
    }
    pub(crate) fn verification_site(&self) -> Option<(usize, usize)> {
        self.verification_site
            .and_then(|site| site.chunk.map(|chunk| (chunk, site.instruction)))
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.kind == ErrorKind::Parse {
            if let Some(position) = self.position() {
                return write!(
                    f,
                    "{} error (line {}): {}",
                    self.kind, position.line, self.message
                );
            }
        }
        if let Some(script_error) = &self.script_error {
            if script_error.code.as_ref() == "throw" {
                return write!(f, "{} error: {}", self.kind, self.message);
            }
            return write!(
                f,
                "{} error [{}]: {}",
                self.kind, script_error.code, self.message
            );
        }
        write!(f, "{} error: {}", self.kind, self.message)
    }
}
impl std::error::Error for Error {}

/// A cloneable, one-way cancellation signal owned by the embedding host.
///
/// Clones share state. Cancelling a token causes the next VM instruction check
/// in every context configured with it to stop with [`ResourceLimit::Cancellation`].
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    /// Creates an uncancelled token.
    pub fn new() -> Self {
        Self::default()
    }
    /// Requests cancellation for all contexts sharing this token.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    /// Reports whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Type-erased host state owned by one execution Context.
///
/// The state is never exposed to QuickCoffee code, included in retained-memory
/// accounting, or stored in Runtime compilation caches. Interior mutability is
/// explicit in the host-provided type, such as `RefCell<T>`.
#[derive(Clone)]
pub struct HostState(Rc<dyn Any>);
impl HostState {
    /// Wraps one same-thread `'static` host value.
    pub fn new<T: 'static>(value: T) -> Self {
        Self(Rc::new(value))
    }

    /// Returns the state when its concrete host type matches `T`.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.0.downcast_ref()
    }

    /// Clones and downcasts the shared state handle when its type matches `T`.
    pub fn downcast<T: 'static>(&self) -> Option<Rc<T>> {
        self.0.clone().downcast().ok()
    }

    /// Reports whether this state contains `T`.
    pub fn is<T: 'static>(&self) -> bool {
        self.0.is::<T>()
    }
}

/// Auditable categories for explicitly installed host capabilities.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum CapabilityKind {
    /// A host-selected clock or deterministic time source.
    Clock,
    /// A host-selected random or deterministic entropy source.
    Random,
    /// A host-selected logging or audit sink.
    Logging,
    /// A host-selected filesystem authority.
    File,
    /// A host-selected network authority.
    Network,
}

/// A typed, script-invisible key for one host capability allowlist slot.
///
/// The category and static name identify the slot. `T` is checked when a host
/// callback retrieves the opaque handle; constructing the same slot with a
/// different `T` therefore returns `None` rather than exposing the value.
pub struct CapabilityKey<T: 'static> {
    kind: CapabilityKind,
    name: &'static str,
    marker: PhantomData<fn() -> T>,
}
impl<T: 'static> CapabilityKey<T> {
    /// Defines a typed capability slot without installing any authority.
    pub const fn new(kind: CapabilityKind, name: &'static str) -> Self {
        Self {
            kind,
            name,
            marker: PhantomData,
        }
    }

    /// Returns the auditable capability category.
    pub const fn kind(self) -> CapabilityKind {
        self.kind
    }

    /// Returns the host-defined static slot name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    fn id(self) -> CapabilityId {
        CapabilityId {
            kind: self.kind,
            name: self.name,
        }
    }
}
impl<T: 'static> Clone for CapabilityKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: 'static> Copy for CapabilityKey<T> {}
impl<T: 'static> fmt::Debug for CapabilityKey<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityKey")
            .field("kind", &self.kind)
            .field("name", &self.name)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CapabilityId {
    kind: CapabilityKind,
    name: &'static str,
}

/// A Context-owned allowlist of typed, script-invisible host capabilities.
///
/// The table owns no ambient authority by default. Values stay outside script
/// globals, serialization, Runtime compilation caches, and managed-memory
/// census. Contextual callbacks explicitly account capability work through
/// [`NativeCallContext`] fuel, cancellation, and allocation APIs.
#[derive(Clone, Default)]
pub struct HostCapabilities {
    entries: BTreeMap<CapabilityId, HostState>,
}
impl HostCapabilities {
    /// Creates an empty capability allowlist.
    pub fn new() -> Self {
        Self::default()
    }

    /// Installs or replaces one typed capability slot.
    pub fn insert<T: 'static>(&mut self, key: CapabilityKey<T>, capability: T) {
        self.entries.insert(key.id(), HostState::new(capability));
    }

    /// Clones the opaque capability handle when the slot and type match.
    pub fn get<T: 'static>(&self, key: CapabilityKey<T>) -> Option<Rc<T>> {
        self.entries.get(&key.id())?.downcast()
    }

    /// Reports whether a slot exists with the requested concrete type.
    pub fn contains<T: 'static>(&self, key: CapabilityKey<T>) -> bool {
        self.entries
            .get(&key.id())
            .is_some_and(|capability| capability.is::<T>())
    }

    /// Removes a slot only when its stored concrete type matches `T`.
    pub fn remove<T: 'static>(&mut self, key: CapabilityKey<T>) -> bool {
        let id = key.id();
        if !self
            .entries
            .get(&id)
            .is_some_and(|capability| capability.is::<T>())
        {
            return false;
        }
        self.entries.remove(&id);
        true
    }

    /// Removes every installed capability.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns the number of allowlisted slots.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Reports whether the allowlist contains no slots.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterates installed category/name descriptors in deterministic order.
    ///
    /// Concrete host types and values remain opaque.
    pub fn descriptors(
        &self,
    ) -> impl ExactSizeIterator<Item = (CapabilityKind, &'static str)> + '_ {
        self.entries.keys().map(|id| (id.kind, id.name))
    }
}

#[derive(Clone, Default)]
struct HostBindings {
    state: Option<HostState>,
    capabilities: HostCapabilities,
}
impl HostBindings {
    fn is_empty(&self) -> bool {
        self.state.is_none() && self.capabilities.is_empty()
    }
}

trait HostBindingsView {
    fn state(&self) -> Option<&HostState>;
    fn capabilities(&self) -> &HostCapabilities;
}
impl HostBindingsView for HostBindings {
    fn state(&self) -> Option<&HostState> {
        self.state.as_ref()
    }
    fn capabilities(&self) -> &HostCapabilities {
        &self.capabilities
    }
}
// Keep the execution-facing handle pointer-wide like the legacy `HostState`
// trait object. A thin handle measurably perturbs the adjacent VM hot layout;
// Context storage remains a thin optional `Rc<HostBindings>` when configured.
type HostBindingsViewHandle = Rc<dyn HostBindingsView>;

/// Per-invocation controls available to an opt-in contextual native callback.
pub struct NativeCallContext {
    cancellation: Option<CancellationToken>,
    resource_limits: ResourceLimits,
    host_bindings: Option<HostBindingsViewHandle>,
    fuel_remaining: u64,
    managed_objects_allocated: u64,
    managed_bytes_allocated: u64,
}
impl NativeCallContext {
    /// Returns the deterministic value-size policy for the current execution.
    pub fn resource_limits(&self) -> ResourceLimits {
        self.resource_limits
    }

    /// Returns fuel left after charges already made by this callback.
    pub fn fuel_remaining(&self) -> u64 {
        self.fuel_remaining
    }

    /// Charges deterministic host work against the current execution's fuel.
    ///
    /// Insufficient fuel consumes the remainder and returns an uncatchable
    /// [`ResourceLimit::Fuel`] error. Charges remain visible if the callback
    /// subsequently returns another error.
    pub fn consume_fuel(&mut self, amount: u64) -> Result<(), Error> {
        let Some(remaining) = self.fuel_remaining.checked_sub(amount) else {
            self.fuel_remaining = 0;
            return Err(Error::resource(
                ResourceLimit::Fuel,
                "execution fuel exhausted by host callback",
            ));
        };
        self.fuel_remaining = remaining;
        Ok(())
    }

    /// Reports whether the embedding host requested cancellation.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
    }

    /// Returns an uncatchable cancellation error when cancellation was requested.
    pub fn check_cancelled(&self) -> Result<(), Error> {
        if self.is_cancelled() {
            Err(Error::resource(
                ResourceLimit::Cancellation,
                "execution cancelled by host",
            ))
        } else {
            Ok(())
        }
    }

    /// Clones the Context-owned state handle when its concrete type matches `T`.
    pub fn host_state<T: 'static>(&self) -> Option<Rc<T>> {
        self.host_bindings.as_ref()?.state()?.downcast()
    }

    /// Clones an allowlisted opaque capability when its slot and type match.
    pub fn capability<T: 'static>(&self, key: CapabilityKey<T>) -> Option<Rc<T>> {
        self.host_bindings.as_ref()?.capabilities().get(key)
    }

    /// Records logical managed allocation performed by host work.
    ///
    /// These counters saturating-add to the current [`ExecutionStats`] even if
    /// the callback returns an error. They do not change legacy
    /// [`ExecutionStats::value_allocations`] and are telemetry rather than a
    /// transient-memory hard limit.
    pub fn record_managed_allocation(&mut self, objects: u64, bytes: u64) {
        self.managed_objects_allocated = self.managed_objects_allocated.saturating_add(objects);
        self.managed_bytes_allocated = self.managed_bytes_allocated.saturating_add(bytes);
    }
}

/// A host callback callable from QuickCoffee code.
pub type NativeFunction = Rc<dyn Fn(&[Value]) -> Result<Value, Error>>;
/// A host callback with cooperative execution controls and typed host state.
pub type ContextualNativeFunction =
    Rc<dyn Fn(&mut NativeCallContext, &[Value]) -> Result<Value, Error>>;

fn native_value(function: NativeFunction) -> Value {
    Value::Function(Rc::new(Function {
        inner: FunctionKind::Native {
            function,
            allocation_profile: None,
        },
    }))
}

fn contextual_native_value(function: ContextualNativeFunction) -> Value {
    Value::Function(Rc::new(Function {
        inner: FunctionKind::ContextualNative { function },
    }))
}
/// Opaque class metadata created only by verified QuickCoffee bytecode.
pub struct Class {
    name: Rc<str>,
    superclass: Option<Rc<Class>>,
    constructor: Option<Rc<Function>>,
    instance_methods: BTreeMap<String, Rc<Function>>,
    static_methods: BTreeMap<String, Rc<Function>>,
    static_fields: RefCell<BTreeMap<String, Value>>,
}
/// Opaque instance state owned by one QuickCoffee class.
pub struct Instance {
    class: Rc<Class>,
    fields: RefCell<BTreeMap<String, Value>>,
}
/// Opaque callable values are constructed by QuickCoffee or Context native registration APIs.
pub struct Function {
    inner: FunctionKind,
}
enum FunctionKind {
    Bytecode {
        params: Vec<Pattern>,
        required: usize,
        rest: Option<String>,
        receiver: bool,
        chunk: Rc<Chunk>,
        debug_info: Option<Rc<ProgramDebugInfo>>,
        execution_plan: Option<Rc<ProgramExecutionPlan>>,
        fast_parameters: Option<Vec<Option<usize>>>,
        env: Env,
    },
    Native {
        function: NativeFunction,
        allocation_profile: Option<fn(&[Value], &Value) -> ManagedAllocation>,
    },
    ResourceBuiltin {
        function: fn(&[Value], ResourceLimits) -> Result<Value, Error>,
        allocation_profile: Option<fn(&[Value], &Value) -> ManagedAllocation>,
    },
    BoundMethod {
        function: Rc<Function>,
        receiver: Value,
        context: MethodContext,
    },
    UnboundMethod {
        owner: Rc<str>,
        name: Rc<str>,
    },
    // Keep uncommon receiver binding out of the ordinary bytecode function
    // layout and call path.
    ReceiverBound {
        function: Rc<Function>,
        captured_receiver: Option<Value>,
    },
    // Append opt-in embedding variants so existing callable discriminants and
    // their ordinary dispatch paths remain stable.
    ContextualNative {
        function: ContextualNativeFunction,
    },
}
#[derive(Clone)]
struct MethodContext {
    owner: Rc<Class>,
    name: Rc<str>,
    kind: MethodKind,
}
#[derive(Clone, Copy)]
enum MethodKind {
    Constructor,
    Instance,
    Static,
}
type Env = Rc<RefCell<Environment>>;
struct Environment {
    indices: BTreeMap<Rc<str>, usize>,
    slots: Vec<(Rc<str>, Value)>,
    initialized: Vec<bool>,
    parent: Option<Env>,
}
// Pattern binding is atomic, so both the name index and its stable slots must
// roll back together when a nested/default pattern fails.
#[derive(Clone)]
struct EnvironmentSnapshot {
    indices: BTreeMap<Rc<str>, usize>,
    slots: Vec<(Rc<str>, Value)>,
    initialized: Vec<bool>,
}
impl Environment {
    fn get_local(&self, name: &str) -> Option<Value> {
        let slot = *self.indices.get(name)?;
        self.slots.get(slot).map(|(_, value)| value.clone())
    }

    fn get_local_with_slot(&self, name: &str) -> Option<(usize, Value)> {
        let slot = *self.indices.get(name)?;
        self.slots.get(slot).map(|(_, value)| (slot, value.clone()))
    }

    fn get_cached(&self, name: &str, slot: usize) -> Option<Value> {
        self.slots
            .get(slot)
            .filter(|(stored, _)| stored.as_ref() == name)
            .map(|(_, value)| value.clone())
    }

    fn get_shared_cached(&self, name: &str, slot: usize) -> Option<Value> {
        self.initialized
            .get(slot)
            .copied()
            .unwrap_or(true)
            .then_some(())?;
        self.get_cached(name, slot)
    }

    fn get_resolved(&self, slot: usize) -> Option<Option<Value>> {
        let (_, value) = self.slots.get(slot)?;
        Some(
            self.initialized
                .get(slot)
                .copied()
                .unwrap_or(true)
                .then(|| value.clone()),
        )
    }

    fn set_resolved(&mut self, slot: usize, value: Value) -> Result<(), Value> {
        if slot >= self.slots.len() {
            return Err(value);
        }
        if let Some(initialized) = self.initialized.get_mut(slot) {
            if !*initialized {
                *initialized = true;
                let name = self.slots[slot].0.clone();
                self.indices.insert(name, slot);
            }
        }
        self.slots[slot].1 = value;
        Ok(())
    }

    fn set_local(&mut self, name: &str, value: Value) -> usize {
        if let Some(slot) = self.indices.get(name).copied() {
            self.slots[slot].1 = value;
            return slot;
        }
        if !self.initialized.is_empty() {
            if let Some(slot) = self
                .slots
                .iter()
                .enumerate()
                .position(|(slot, (stored, _))| !self.initialized[slot] && stored.as_ref() == name)
            {
                self.initialized[slot] = true;
                let name = self.slots[slot].0.clone();
                self.indices.insert(name, slot);
                self.slots[slot].1 = value;
                return slot;
            }
        }
        let slot = self.slots.len();
        let name: Rc<str> = Rc::from(name);
        self.indices.insert(name.clone(), slot);
        self.slots.push((name, value));
        if !self.initialized.is_empty() {
            self.initialized.push(true);
        }
        slot
    }

    fn set_cached(&mut self, name: &str, slot: usize, value: Value) -> Result<(), Value> {
        match self.slots.get_mut(slot) {
            Some((stored, current)) if stored.as_ref() == name => {
                *current = value;
                Ok(())
            }
            _ => Err(value),
        }
    }

    fn set_shared_cached(&mut self, name: &str, slot: usize, value: Value) -> Result<(), Value> {
        if self
            .slots
            .get(slot)
            .is_none_or(|(stored, _)| stored.as_ref() != name)
        {
            return Err(value);
        }
        if let Some(initialized) = self.initialized.get_mut(slot) {
            if !*initialized {
                *initialized = true;
                let name = self.slots[slot].0.clone();
                self.indices.insert(name, slot);
            }
        }
        self.slots[slot].1 = value;
        Ok(())
    }

    fn snapshot(&self) -> EnvironmentSnapshot {
        EnvironmentSnapshot {
            indices: self.indices.clone(),
            slots: self.slots.clone(),
            initialized: self.initialized.clone(),
        }
    }

    fn restore(&mut self, snapshot: EnvironmentSnapshot) {
        self.indices = snapshot.indices;
        self.slots = snapshot.slots;
        self.initialized = snapshot.initialized;
    }
}
fn env(parent: Option<Env>) -> Env {
    Rc::new(RefCell::new(Environment {
        indices: BTreeMap::new(),
        slots: vec![],
        initialized: vec![],
        parent,
    }))
}
fn env_with_unset_slots(parent: Env, names: &[Rc<str>]) -> Env {
    let slots = names
        .iter()
        .cloned()
        .map(|name| (name, Value::Nil))
        .collect();
    Rc::new(RefCell::new(Environment {
        indices: BTreeMap::new(),
        slots,
        initialized: vec![false; names.len()],
        parent: Some(parent),
    }))
}
fn lookup(e: &Env, n: &str) -> Option<Value> {
    let b = e.borrow();
    if let Some(value) = b.get_local(n) {
        return Some(value);
    }
    let p = b.parent.clone();
    drop(b);
    p.and_then(|p| lookup(&p, n))
}

/// A reusable compiler that does not hold execution state.
#[derive(Clone, Default)]
pub struct Engine;

const DEFAULT_PROGRAM_CACHE_ENTRIES: usize = 64;
const DEFAULT_MODULE_CACHE_ENTRIES: usize = 64;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SourceCacheKey {
    name: Option<String>,
    source: String,
}

struct CompileCache<T> {
    capacity: usize,
    entries: BTreeMap<SourceCacheKey, T>,
    order: VecDeque<SourceCacheKey>,
    hits: u64,
    misses: u64,
    evictions: u64,
}
impl<T: Clone> CompileCache<T> {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: BTreeMap::new(),
            order: VecDeque::new(),
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    fn get(&mut self, key: &SourceCacheKey) -> Option<T> {
        let Some(value) = self.entries.get(key).cloned() else {
            self.misses = self.misses.saturating_add(1);
            return None;
        };
        self.hits = self.hits.saturating_add(1);
        if let Some(index) = self.order.iter().position(|candidate| candidate == key) {
            self.order.remove(index);
        }
        self.order.push_back(key.clone());
        Some(value)
    }

    fn insert(&mut self, key: SourceCacheKey, value: T) {
        if self.capacity == 0 {
            return;
        }
        if self.entries.contains_key(&key) {
            self.entries.insert(key.clone(), value);
            if let Some(index) = self.order.iter().position(|candidate| candidate == &key) {
                self.order.remove(index);
            }
            self.order.push_back(key);
            return;
        }
        while self.entries.len() >= self.capacity {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if self.entries.remove(&oldest).is_some() {
                self.evictions = self.evictions.saturating_add(1);
            }
        }
        self.entries.insert(key.clone(), value);
        self.order.push_back(key);
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
}

struct RuntimeInner {
    engine: Engine,
    programs: RefCell<CompileCache<Program>>,
    modules: RefCell<CompileCache<Module>>,
}

/// A same-thread owner for shared immutable compilation artifacts.
///
/// Clones share bounded Program and Module compilation caches. Contexts made by
/// one Runtime keep all mutable script state isolated: globals, module exports,
/// fuel, cancellation, statistics, and retained-memory accounting are never
/// stored in the Runtime. The current VM uses `Rc`, so Runtime is deliberately
/// not `Send` or `Sync`.
#[derive(Clone)]
pub struct Runtime(Rc<RuntimeInner>);

/// Configures a [`Runtime`] and its bounded compilation caches.
pub struct RuntimeBuilder {
    program_cache_entries: usize,
    module_cache_entries: usize,
}

/// A read-only cumulative snapshot of one Runtime's compilation caches.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeCacheStats {
    /// Programs currently retained by the Runtime cache.
    pub program_entries: usize,
    /// Successful Program cache lookups.
    pub program_hits: u64,
    /// Program cache lookups that required compilation.
    pub program_misses: u64,
    /// Programs removed by the configured capacity boundary.
    pub program_evictions: u64,
    /// Modules currently retained by the Runtime cache.
    pub module_entries: usize,
    /// Successful Module cache lookups.
    pub module_hits: u64,
    /// Module cache lookups that required compilation.
    pub module_misses: u64,
    /// Modules removed by the configured capacity boundary.
    pub module_evictions: u64,
}
/// A reference-counted compiled program for repeated execution.
///
/// The shared storage is private so embedding callers do not need to manage
/// `Rc` themselves; cloning a `Program` is cheap and does not copy bytecode.
/// Programs produced by [`Engine::compile_program`] are verified immediately;
/// programs wrapped from a raw [`Chunk`] verify on their first execution.
#[derive(Debug)]
struct ProgramInner {
    chunk: Rc<Chunk>,
    verified: Cell<bool>,
    debug_info: Option<Rc<ProgramDebugInfo>>,
    execution_plan: Option<Rc<ProgramExecutionPlan>>,
}
#[derive(Debug)]
struct ProgramExecutionPlan {
    chunks: BTreeMap<usize, Rc<ChunkBindingSlots>>,
}
#[derive(Debug)]
struct ChunkBindingSlots {
    local_names: Vec<Rc<str>>,
    local_by_pc: Vec<Option<usize>>,
    isolated_frame: bool,
    shared_environment: bool,
    // Unresolved/global names retain the guarded hint path because Programs
    // are shared across Contexts with independently ordered environments.
    cached_by_pc: Vec<Cell<Option<usize>>>,
}
impl ChunkBindingSlots {
    fn fast_parameter_slots(
        &self,
        params: &[Pattern],
        required: usize,
        rest: Option<&str>,
    ) -> Option<Vec<Option<usize>>> {
        if !self.isolated_frame || required != params.len() || rest.is_some() {
            return None;
        }
        let mut slots = Vec::with_capacity(params.len());
        for pattern in params {
            match pattern {
                Pattern::Bind(name) => slots.push(Some(
                    self.local_names
                        .binary_search_by(|candidate| candidate.as_ref().cmp(name))
                        .ok()?,
                )),
                Pattern::Ignore => slots.push(None),
                _ => return None,
            }
        }
        Some(slots)
    }
}
impl ProgramExecutionPlan {
    fn new(chunk: &Rc<Chunk>) -> Self {
        let mut chunks = BTreeMap::new();
        Self::register_chunk(chunk, BTreeSet::new(), &mut chunks);
        Self { chunks }
    }

    fn register_chunk(
        chunk: &Rc<Chunk>,
        mut local_names: BTreeSet<String>,
        chunks: &mut BTreeMap<usize, Rc<ChunkBindingSlots>>,
    ) {
        let key = Rc::as_ptr(chunk) as usize;
        if chunks.contains_key(&key) {
            return;
        }
        for instruction in &chunk.code {
            match instruction {
                Instruction::Store(name) => {
                    local_names.insert(name.clone());
                }
                Instruction::Try { name, .. } => {
                    if name != "_" {
                        local_names.insert(name.clone());
                    }
                }
                Instruction::Destructure(pattern) => {
                    Self::collect_pattern_bindings(pattern, &mut local_names)
                }
                Instruction::IterNext { patterns, .. } => {
                    for pattern in patterns {
                        Self::collect_pattern_bindings(pattern, &mut local_names);
                    }
                }
                _ => {}
            }
        }
        let local_names = local_names.into_iter().map(Rc::from).collect::<Vec<_>>();
        let local_indices = local_names
            .iter()
            .cloned()
            .enumerate()
            .map(|(slot, name)| (name, slot))
            .collect::<BTreeMap<Rc<str>, usize>>();
        let local_by_pc = chunk
            .code
            .iter()
            .map(|instruction| match instruction {
                Instruction::Load(name)
                | Instruction::LoadOrNil(name)
                | Instruction::Store(name) => local_indices.get(name.as_str()).copied(),
                _ => None,
            })
            .collect();
        // A direct call cannot observe its caller's lexical frame: bytecode
        // functions carry the environment captured where they were created,
        // and native functions receive values only. Keep spread calls out of
        // this first extension while their argument carrier is investigated.
        let shared_environment = chunk.code.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::MakeFunction(_) | Instruction::MakeBoundFunction(_)
            )
        });
        let isolated_frame = !chunk.code.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::Destructure(_)
                    | Instruction::IterNext { .. }
                    | Instruction::Try { .. }
                    | Instruction::EndTry
                    | Instruction::MakeFunction(_)
                    | Instruction::MakeBoundFunction(_)
                    | Instruction::CallSpread
            )
        });
        chunks.insert(
            key,
            Rc::new(ChunkBindingSlots {
                local_names,
                local_by_pc,
                isolated_frame,
                shared_environment,
                cached_by_pc: (0..chunk.code.len()).map(|_| Cell::new(None)).collect(),
            }),
        );
        for constant in &chunk.constants {
            if let Constant::Function {
                params,
                rest,
                chunk,
                ..
            } = constant
            {
                let mut function_locals = BTreeSet::new();
                for pattern in params {
                    Self::collect_pattern_bindings(pattern, &mut function_locals);
                    Self::register_pattern(pattern, chunks);
                }
                if let Some(rest) = rest {
                    function_locals.insert(rest.clone());
                }
                Self::register_chunk(chunk, function_locals, chunks);
            }
        }
        for instruction in &chunk.code {
            match instruction {
                Instruction::Destructure(pattern) => Self::register_pattern(pattern, chunks),
                Instruction::IterNext { patterns, .. } => {
                    for pattern in patterns {
                        Self::register_pattern(pattern, chunks);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_pattern_bindings(pattern: &Pattern, names: &mut BTreeSet<String>) {
        match pattern {
            Pattern::Bind(name) | Pattern::Rest(name) => {
                if name != "_" {
                    names.insert(name.clone());
                }
            }
            Pattern::Default { pattern, .. } => Self::collect_pattern_bindings(pattern, names),
            Pattern::Array(patterns) => {
                for pattern in patterns {
                    Self::collect_pattern_bindings(pattern, names);
                }
            }
            Pattern::Map(fields) => {
                for (_, pattern) in fields {
                    Self::collect_pattern_bindings(pattern, names);
                }
            }
            Pattern::MapRest { fields, rest } => {
                for (_, pattern) in fields {
                    Self::collect_pattern_bindings(pattern, names);
                }
                if rest != "_" {
                    names.insert(rest.clone());
                }
            }
            Pattern::Ignore => {}
        }
    }

    fn register_pattern(pattern: &Pattern, chunks: &mut BTreeMap<usize, Rc<ChunkBindingSlots>>) {
        match pattern {
            Pattern::Default { pattern, default } => {
                Self::register_pattern(pattern, chunks);
                Self::register_chunk(default, BTreeSet::new(), chunks);
            }
            Pattern::Array(patterns) => {
                for pattern in patterns {
                    Self::register_pattern(pattern, chunks);
                }
            }
            Pattern::Map(fields) | Pattern::MapRest { fields, .. } => {
                for (_, pattern) in fields {
                    Self::register_pattern(pattern, chunks);
                }
            }
            Pattern::Ignore | Pattern::Bind(_) | Pattern::Rest(_) => {}
        }
    }

    fn slots(&self, chunk: &Rc<Chunk>) -> Option<Rc<ChunkBindingSlots>> {
        self.chunks
            .get(&(Rc::as_ptr(chunk) as usize))
            .map(Rc::clone)
    }
}
#[derive(Debug)]
struct ProgramDebugInfo {
    source_name: Option<Rc<str>>,
    instruction_spans: BTreeMap<usize, ChunkSourceMap>,
}
impl ProgramDebugInfo {
    fn new(chunk: &Rc<Chunk>, source_map: CompiledSourceMap, source_name: Option<&str>) -> Self {
        let mut instruction_spans = BTreeMap::new();
        instruction_spans.insert(Rc::as_ptr(chunk) as usize, source_map.top);
        for (nested, source_map) in source_map.nested {
            instruction_spans.insert(Rc::as_ptr(&nested) as usize, source_map);
        }
        Self {
            source_name: source_name.map(Rc::from),
            instruction_spans,
        }
    }
    fn span(&self, chunk: &Rc<Chunk>, pc: usize) -> Option<SourceSpan> {
        let source_map = self.instruction_spans.get(&(Rc::as_ptr(chunk) as usize))?;
        let span_id = *source_map.instructions.get(pc)?;
        if span_id == 0 {
            return None;
        }
        let span = *source_map.spans.get(span_id as usize - 1)?;
        let mut span = span.into_source_span();
        span.source_name = self.source_name.as_deref().map(str::to_owned);
        Some(span)
    }
}
/// A cheaply cloneable, verified bytecode program for repeated execution.
#[derive(Clone, Debug)]
pub struct Program(Rc<ProgramInner>);
impl From<Chunk> for Program {
    fn from(chunk: Chunk) -> Self {
        Self(Rc::new(ProgramInner {
            chunk: Rc::new(chunk),
            verified: Cell::new(false),
            debug_info: None,
            execution_plan: None,
        }))
    }
}
impl Program {
    pub(crate) fn from_compiled(
        chunk: Chunk,
        source_map: CompiledSourceMap,
        source_name: Option<&str>,
    ) -> Self {
        let chunk = Rc::new(chunk);
        let debug_info = Rc::new(ProgramDebugInfo::new(&chunk, source_map, source_name));
        let execution_plan = Rc::new(ProgramExecutionPlan::new(&chunk));
        Self(Rc::new(ProgramInner {
            chunk,
            verified: Cell::new(true),
            debug_info: Some(debug_info),
            execution_plan: Some(execution_plan),
        }))
    }
    #[cfg(test)]
    fn without_binding_slots(&self) -> Self {
        Self(Rc::new(ProgramInner {
            chunk: Rc::clone(&self.0.chunk),
            verified: Cell::new(self.0.verified.get()),
            debug_info: self.0.debug_info.clone(),
            execution_plan: None,
        }))
    }
    /// Verifies the program and caches a successful result.
    pub fn verify(&self) -> Result<(), Error> {
        let result = self.0.chunk.verify();
        if result.is_ok() {
            self.0.verified.set(true);
        }
        result
    }
    /// Returns a human-readable disassembly of the shared bytecode.
    pub fn disassemble(&self) -> String {
        self.0.chunk.disassemble()
    }
    /// Returns the deterministic fingerprint of the shared bytecode.
    pub fn fingerprint(&self) -> u64 {
        self.0.chunk.fingerprint()
    }
    fn ensure_verified(&self) -> Result<(), Error> {
        if self.0.verified.get() {
            Ok(())
        } else {
            self.verify()
        }
    }
}
impl Engine {
    /// Creates a stateless compiler.
    pub fn new() -> Self {
        Self
    }
    /// Compiles and verifies source into an owned bytecode chunk.
    pub fn compile(&self, source: &str) -> Result<Chunk, Error> {
        compile(source)
    }
    /// Compiles and verifies source while attaching an opaque host-provided
    /// name to any source labels produced on failure.
    pub fn compile_named(&self, source_name: &str, source: &str) -> Result<Chunk, Error> {
        crate::compile_named(source_name, source)
    }
    /// Compiles source into cheaply cloneable shared bytecode.
    pub fn compile_program(&self, source: &str) -> Result<Program, Error> {
        self.compile_program_source(None, source)
    }
    /// Compiles named source into cheaply cloneable shared bytecode.
    /// A name ending in `.litcoffee` enables literate CoffeeScript preprocessing.
    pub fn compile_program_named(&self, source_name: &str, source: &str) -> Result<Program, Error> {
        self.compile_program_source(Some(source_name), source)
    }
    /// Compiles and verifies source without executing it, collecting every
    /// parser error recoverable at a top-level statement boundary.
    ///
    /// Lexing, lowering, and verification stop at their first error. Existing
    /// [`Self::compile_program`] methods retain their first-error behavior.
    pub fn check_program(&self, source: &str) -> Result<(), Vec<Error>> {
        self.check_program_source(None, source)
    }
    /// Like [`Self::check_program`], while attaching the caller-provided
    /// opaque source name to every returned diagnostic label. A name ending in
    /// `.litcoffee` enables literate CoffeeScript preprocessing.
    pub fn check_program_named(&self, source_name: &str, source: &str) -> Result<(), Vec<Error>> {
        self.check_program_source(Some(source_name), source)
    }
    fn compile_program_source(
        &self,
        source_name: Option<&str>,
        source: &str,
    ) -> Result<Program, Error> {
        let attach_name = |error: Error| match source_name {
            Some(source_name) => error.with_source_name(source_name),
            None => error,
        };
        let prepared = crate::source::prepare(source_name, source).map_err(attach_name)?;
        let ast = parser::parse_with_columns(&prepared.text, prepared.columns_are_precise)
            .map_err(attach_name)?;
        let (chunk, source_map) = lowering::compile_mapped(&ast).map_err(attach_name)?;
        lowering::verify_mapped(&chunk, &source_map).map_err(attach_name)?;
        Ok(Program::from_compiled(chunk, source_map, source_name))
    }
    fn check_program_source(
        &self,
        source_name: Option<&str>,
        source: &str,
    ) -> Result<(), Vec<Error>> {
        let attach_name = |error: Error| match source_name {
            Some(source_name) => error.with_source_name(source_name),
            None => error,
        };
        let prepared = crate::source::prepare(source_name, source)
            .map_err(|error| vec![attach_name(error)])?;
        let ast = parser::parse_recover_with_columns(&prepared.text, prepared.columns_are_precise)
            .map_err(|errors| errors.into_iter().map(attach_name).collect::<Vec<Error>>())?;
        let (chunk, source_map) =
            lowering::compile_mapped(&ast).map_err(|error| vec![attach_name(error)])?;
        lowering::verify_mapped(&chunk, &source_map).map_err(|error| vec![attach_name(error)])?;
        Ok(())
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self {
            program_cache_entries: DEFAULT_PROGRAM_CACHE_ENTRIES,
            module_cache_entries: DEFAULT_MODULE_CACHE_ENTRIES,
        }
    }
}
impl RuntimeBuilder {
    /// Creates a builder with 64 Program and 64 Module cache entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum number of shared Programs retained by the Runtime.
    ///
    /// Zero disables this cache. The capacity counts entries, not source or
    /// bytecode bytes; byte-oriented compile limits remain a separate policy.
    pub fn program_cache_entries(mut self, entries: usize) -> Self {
        self.program_cache_entries = entries;
        self
    }

    /// Sets the maximum number of shared Modules retained by the Runtime.
    ///
    /// Zero disables this cache. Module evaluation results are never cached.
    pub fn module_cache_entries(mut self, entries: usize) -> Self {
        self.module_cache_entries = entries;
        self
    }

    /// Builds a Runtime with independent bounded Program and Module caches.
    pub fn build(self) -> Runtime {
        Runtime(Rc::new(RuntimeInner {
            engine: Engine::new(),
            programs: RefCell::new(CompileCache::new(self.program_cache_entries)),
            modules: RefCell::new(CompileCache::new(self.module_cache_entries)),
        }))
    }
}

impl Default for Runtime {
    fn default() -> Self {
        RuntimeBuilder::new().build()
    }
}
impl Runtime {
    /// Creates a same-thread Runtime with default bounded compilation caches.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts configuration of a Runtime and its compilation-cache capacities.
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    /// Starts configuration of an isolated Context owned by this Runtime.
    pub fn context_builder(&self) -> ContextBuilder {
        ContextBuilder::new(self.clone())
    }

    /// Creates an isolated Context with the default execution policies.
    pub fn new_context(&self) -> Context {
        self.context_builder().build()
    }

    /// Compiles source into a verified Program, reusing an exact cache entry.
    pub fn compile_program(&self, source: &str) -> Result<Program, Error> {
        self.compile_program_source(None, source)
    }

    /// Compiles named source into a verified Program, reusing an exact cache entry.
    ///
    /// The complete source name and raw UTF-8 source form the cache identity. A
    /// name ending in `.litcoffee` enables literate CoffeeScript preprocessing.
    pub fn compile_program_named(&self, source_name: &str, source: &str) -> Result<Program, Error> {
        self.compile_program_source(Some(source_name), source)
    }

    fn compile_program_source(
        &self,
        source_name: Option<&str>,
        source: &str,
    ) -> Result<Program, Error> {
        let key = SourceCacheKey {
            name: source_name.map(str::to_owned),
            source: source.to_owned(),
        };
        if let Some(program) = self.0.programs.borrow_mut().get(&key) {
            return Ok(program);
        }
        let program = match source_name {
            Some(source_name) => self.0.engine.compile_program_named(source_name, source)?,
            None => self.0.engine.compile_program(source)?,
        };
        self.0.programs.borrow_mut().insert(key, program.clone());
        Ok(program)
    }

    /// Returns cumulative cache counters and current entry counts.
    pub fn cache_stats(&self) -> RuntimeCacheStats {
        let programs = self.0.programs.borrow();
        let modules = self.0.modules.borrow();
        RuntimeCacheStats {
            program_entries: programs.entries.len(),
            program_hits: programs.hits,
            program_misses: programs.misses,
            program_evictions: programs.evictions,
            module_entries: modules.entries.len(),
            module_hits: modules.hits,
            module_misses: modules.misses,
            module_evictions: modules.evictions,
        }
    }

    /// Removes cached compilation artifacts without changing cumulative counters.
    ///
    /// Existing Program and Module handles remain valid because they own shared
    /// immutable storage independently of the cache.
    pub fn clear_compile_caches(&self) {
        self.0.programs.borrow_mut().clear();
        self.0.modules.borrow_mut().clear();
    }

    /// Returns the Runtime's stateless compiler for uncached Chunk, check, and
    /// module-graph fingerprint operations.
    pub fn engine(&self) -> &Engine {
        &self.0.engine
    }

    pub(crate) fn cached_module(&self, name: &str, source: &str) -> Option<Module> {
        let key = SourceCacheKey {
            name: Some(name.to_owned()),
            source: source.to_owned(),
        };
        self.0.modules.borrow_mut().get(&key)
    }

    pub(crate) fn cache_module(&self, module: Module, source: &str) {
        let key = SourceCacheKey {
            name: Some(module.name().to_owned()),
            source: source.to_owned(),
        };
        self.0.modules.borrow_mut().insert(key, module);
    }
}

/// Public counters for the most recent bytecode execution in a context.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExecutionStats {
    /// Number of VM instructions attempted, including the instruction that
    /// produced a runtime error.
    pub instructions: u64,
    /// Fuel left after the execution stopped.
    pub fuel_remaining: u64,
    /// Greatest nested QuickCoffee function-call depth reached during the run.
    ///
    /// The top-level program does not count toward this value.
    pub call_depth_peak: usize,
    /// Bytecode name loads attempted during the run.
    pub name_loads: u64,
    /// Bytecode name stores attempted during the run.
    pub name_stores: u64,
    /// Bytecode call instructions attempted during the run.
    pub calls: u64,
    /// Bytecode container construction and access instructions attempted during the run.
    pub container_ops: u64,
    /// Bytecode iterator setup, advancement, and cleanup instructions attempted during the run.
    pub iterator_ops: u64,
    /// Bytecode exception-handler and throw instructions attempted during the run.
    pub exception_ops: u64,
    /// Fresh reference-counted value backings created during the run.
    ///
    /// This includes VM and standard-library strings, arrays, maps, and bytecode
    /// functions, but excludes compile-time constants and values allocated by
    /// embedding-host callbacks.
    pub value_allocations: u64,
    /// Lexical environments allocated for QuickCoffee function calls during the run.
    pub environment_allocations: u64,
    /// Logical VM-managed objects allocated during the run.
    ///
    /// This additive counter follows RFC 0146 and is independent of allocator
    /// calls, reference-count headers, pointer width, and object retention.
    pub managed_objects_allocated: u64,
    /// Stable logical payload bytes allocated for [`Self::managed_objects_allocated`].
    ///
    /// This is not RSS, capacity, or a current/peak retained-memory reading.
    pub managed_bytes_allocated: u64,
}

/// A deterministic snapshot of QuickCoffee-managed values retained by one context.
///
/// This follows RFC 0147's logical object and payload-byte model. It is not an
/// allocator, capacity, RSS, or host-object measurement.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RetainedMemory {
    /// Logical managed objects reachable from this context's owned globals.
    pub objects: u64,
    /// Stable logical payload bytes for [`Self::objects`].
    pub bytes: u64,
}

/// Declaratively configures one isolated [`Context`].
///
/// Building more than once is intentionally unsupported because native
/// callbacks may capture non-cloneable host state. Start a new builder from
/// the shared [`Runtime`] for each Context.
pub struct ContextBuilder {
    runtime: Runtime,
    fuel: u64,
    max_call_depth: usize,
    resource_limits: ResourceLimits,
    cancellation: Option<CancellationToken>,
    host_bindings: HostBindings,
    bindings: Vec<(String, Value)>,
}
impl ContextBuilder {
    fn new(runtime: Runtime) -> Self {
        Self {
            runtime,
            fuel: 1_000_000,
            max_call_depth: 1_024,
            resource_limits: ResourceLimits::default(),
            cancellation: None,
            host_bindings: HostBindings::default(),
            bindings: Vec::new(),
        }
    }

    /// Sets the instruction budget for every run in the new Context.
    pub fn fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }

    /// Sets the maximum nested QuickCoffee function-call depth.
    pub fn max_call_depth(mut self, max_call_depth: usize) -> Self {
        self.max_call_depth = max_call_depth;
        self
    }

    /// Sets deterministic data-size and retained-memory policies.
    pub fn resource_limits(mut self, resource_limits: ResourceLimits) -> Self {
        self.resource_limits = resource_limits;
        self
    }

    /// Installs the cancellation token observed by future runs.
    pub fn cancellation_token(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = Some(cancellation);
        self
    }

    /// Installs type-safe host state owned by the new Context.
    pub fn host_state<T: 'static>(mut self, state: T) -> Self {
        self.host_bindings.state = Some(HostState::new(state));
        self
    }

    /// Copies a typed capability allowlist into the new Context.
    pub fn capabilities(mut self, capabilities: HostCapabilities) -> Self {
        self.host_bindings.capabilities = capabilities;
        self
    }

    /// Installs one typed, script-invisible host capability.
    pub fn capability<T: 'static>(mut self, key: CapabilityKey<T>, capability: T) -> Self {
        self.host_bindings.capabilities.insert(key, capability);
        self
    }

    /// Adds an immutable host global to the new Context.
    pub fn global(mut self, name: impl Into<String>, value: Value) -> Self {
        self.bindings.push((name.into(), value));
        self
    }

    /// Adds an opaque host callback to the new Context.
    pub fn native<F>(mut self, name: impl Into<String>, function: F) -> Self
    where
        F: Fn(&[Value]) -> Result<Value, Error> + 'static,
    {
        self.bindings
            .push((name.into(), native_value(Rc::new(function))));
        self
    }

    /// Adds an opaque host callback with cooperative execution controls.
    pub fn contextual_native<F>(mut self, name: impl Into<String>, function: F) -> Self
    where
        F: Fn(&mut NativeCallContext, &[Value]) -> Result<Value, Error> + 'static,
    {
        self.bindings
            .push((name.into(), contextual_native_value(Rc::new(function))));
        self
    }

    /// Builds an isolated Context attached to the configured Runtime.
    pub fn build(self) -> Context {
        let global = BUILTIN_ENVIRONMENT.with(|builtins| env(Some(builtins.clone())));
        let mut context = Context {
            engine: self.runtime.engine().clone(),
            runtime: Some(self.runtime),
            global,
            fuel: self.fuel,
            max_call_depth: self.max_call_depth,
            resource_limits: self.resource_limits,
            cancellation: self.cancellation,
            host_bindings: (!self.host_bindings.is_empty()).then(|| Rc::new(self.host_bindings)),
            last_execution: ExecutionStats::default(),
            retained_memory_high_water: RetainedMemory {
                objects: 1,
                bytes: 0,
            },
        };
        for (name, value) in self.bindings {
            context.set_global(name, value);
        }
        context
    }
}

/// An execution context containing globals, builtins, and per-run resource limits.
pub struct Context {
    engine: Engine,
    runtime: Option<Runtime>,
    global: Env,
    fuel: u64,
    max_call_depth: usize,
    resource_limits: ResourceLimits,
    cancellation: Option<CancellationToken>,
    host_bindings: Option<Rc<HostBindings>>,
    last_execution: ExecutionStats,
    retained_memory_high_water: RetainedMemory,
}

thread_local! {
    // Native builtin functions are immutable. Sharing them through a read-only
    // parent keeps Context construction independent of standard-library size;
    // each Context owns a writable child that can shadow any builtin.
    static BUILTIN_ENVIRONMENT: Env = {
        let global = env(None);
        let mut context = Context {
            engine: Engine::new(),
            runtime: None,
            global: global.clone(),
            fuel: 1_000_000,
            max_call_depth: 1_024,
            resource_limits: ResourceLimits::default(),
            cancellation: None,
            host_bindings: None,
            last_execution: ExecutionStats::default(),
            retained_memory_high_water: RetainedMemory::default(),
        };
        context.install_builtins();
        global
    };
    // Keeping the pool outside `Context`, `Vm`, and `Vm::run` preserves the
    // layout of unrelated dispatch paths. Borrows never cross a script call.
    static REUSABLE_CALL_ARGUMENTS: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
    // Completed bytecode calls return their cleared value-stack capacity here.
    // One bounded buffer removes allocator churn from sequential calls without
    // retaining a stack for every recursion level or changing `Vm` layout.
    static REUSABLE_FRAME_STACK: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ManagedAllocation {
    legacy_value_allocations: u64,
    objects: u64,
    bytes: u64,
}
impl ManagedAllocation {
    fn legacy_shallow(legacy_value_allocations: u64, value: &Value) -> Self {
        let mut allocation = shallow_managed_allocation(value);
        allocation.legacy_value_allocations = legacy_value_allocations;
        allocation
    }

    fn legacy_deep(legacy_value_allocations: u64, value: &Value) -> Self {
        let mut allocation = deep_managed_allocation(value);
        allocation.legacy_value_allocations = legacy_value_allocations;
        allocation
    }

    fn add(&mut self, other: Self) {
        self.legacy_value_allocations = self
            .legacy_value_allocations
            .saturating_add(other.legacy_value_allocations);
        self.objects = self.objects.saturating_add(other.objects);
        self.bytes = self.bytes.saturating_add(other.bytes);
    }
}

const LOGICAL_REFERENCE_BYTES: u64 = 8;
const LOGICAL_MAP_ENTRY_BYTES: u64 = 16;
const LOGICAL_DECIMAL_SCALE_BYTES: u64 = 4;

fn magnitude_bytes(bits: u64) -> u64 {
    bits.saturating_add(7) / 8
}

fn shallow_managed_allocation(value: &Value) -> ManagedAllocation {
    let (objects, bytes) = match value {
        Value::Integer(value) => (1, magnitude_bytes(value.inner().bits())),
        Value::Decimal(value) => (
            1,
            magnitude_bytes(value.inner().bits()).saturating_add(LOGICAL_DECIMAL_SCALE_BYTES),
        ),
        Value::String(value) => (1, value.len() as u64),
        Value::Array(values) => (
            1,
            (values.len() as u64).saturating_mul(LOGICAL_REFERENCE_BYTES),
        ),
        Value::Map(values) => (
            1,
            values.iter().fold(0_u64, |bytes, (key, _)| {
                bytes
                    .saturating_add(LOGICAL_MAP_ENTRY_BYTES)
                    .saturating_add(key.len() as u64)
            }),
        ),
        Value::Error(error) => (
            1,
            (error.code.len() as u64)
                .saturating_add(error.message.len() as u64)
                .saturating_add(LOGICAL_REFERENCE_BYTES.saturating_mul(2)),
        ),
        Value::Class(class) => (
            1,
            (class.name.len() as u64)
                .saturating_add(LOGICAL_REFERENCE_BYTES.saturating_mul(2))
                .saturating_add(
                    (class.instance_methods.len() as u64)
                        .saturating_add(class.static_methods.len() as u64)
                        .saturating_mul(LOGICAL_MAP_ENTRY_BYTES),
                )
                .saturating_add(
                    class
                        .instance_methods
                        .keys()
                        .chain(class.static_methods.keys())
                        .fold(0_u64, |bytes, key| bytes.saturating_add(key.len() as u64)),
                ),
        ),
        Value::Instance(instance) => {
            let fields = instance.fields.borrow();
            (
                1,
                LOGICAL_REFERENCE_BYTES.saturating_add(fields.iter().fold(
                    0_u64,
                    |bytes, (key, _)| {
                        bytes
                            .saturating_add(LOGICAL_MAP_ENTRY_BYTES)
                            .saturating_add(key.len() as u64)
                    },
                )),
            )
        }
        Value::Function(_) => (1, LOGICAL_REFERENCE_BYTES),
        Value::Nil | Value::Bool(_) | Value::Number(_) => (0, 0),
    };
    ManagedAllocation {
        legacy_value_allocations: 0,
        objects,
        bytes,
    }
}

#[derive(Default)]
struct RetainedMemoryCensus {
    snapshot: RetainedMemory,
    integers: BTreeSet<usize>,
    decimals: BTreeSet<usize>,
    strings: BTreeSet<usize>,
    arrays: BTreeSet<usize>,
    maps: BTreeSet<usize>,
    errors: BTreeSet<usize>,
    classes: BTreeSet<usize>,
    instances: BTreeSet<usize>,
    functions: BTreeSet<usize>,
    environments: BTreeSet<usize>,
}
impl RetainedMemoryCensus {
    fn rc_key<T: ?Sized>(value: &Rc<T>) -> usize {
        Rc::as_ptr(value) as *const () as usize
    }

    fn first<T: ?Sized>(seen: &mut BTreeSet<usize>, value: &Rc<T>) -> bool {
        seen.insert(Self::rc_key(value))
    }

    fn add(&mut self, objects: u64, bytes: u64) {
        self.snapshot.objects = self.snapshot.objects.saturating_add(objects);
        self.snapshot.bytes = self.snapshot.bytes.saturating_add(bytes);
    }

    fn value(&mut self, value: &Value) {
        match value {
            Value::Integer(value) => {
                if Self::first(&mut self.integers, value) {
                    self.add(1, magnitude_bytes(value.inner().bits()));
                }
            }
            Value::Decimal(value) => {
                if Self::first(&mut self.decimals, value) {
                    self.add(
                        1,
                        magnitude_bytes(value.inner().bits())
                            .saturating_add(LOGICAL_DECIMAL_SCALE_BYTES),
                    );
                }
            }
            Value::String(value) => {
                if Self::first(&mut self.strings, value) {
                    self.add(1, value.len() as u64);
                }
            }
            Value::Array(values) => {
                if Self::first(&mut self.arrays, values) {
                    self.add(
                        1,
                        (values.len() as u64).saturating_mul(LOGICAL_REFERENCE_BYTES),
                    );
                    for child in values.iter() {
                        self.value(child);
                    }
                }
            }
            Value::Map(values) => {
                if Self::first(&mut self.maps, values) {
                    self.add(
                        1,
                        values.iter().fold(0_u64, |bytes, (key, _)| {
                            bytes
                                .saturating_add(LOGICAL_MAP_ENTRY_BYTES)
                                .saturating_add(key.len() as u64)
                        }),
                    );
                    for child in values.values() {
                        self.value(child);
                    }
                }
            }
            Value::Error(error) => self.error(error),
            Value::Class(class) => self.class(class),
            Value::Instance(instance) => self.instance(instance),
            Value::Function(function) => self.function(function),
            Value::Nil | Value::Bool(_) | Value::Number(_) => {}
        }
    }

    fn error(&mut self, error: &Rc<ScriptError>) {
        if !Self::first(&mut self.errors, error) {
            return;
        }
        self.add(
            1,
            (error.code.len() as u64)
                .saturating_add(error.message.len() as u64)
                .saturating_add(LOGICAL_REFERENCE_BYTES.saturating_mul(2)),
        );
        self.value(&error.data);
        if let Some(cause) = &error.cause {
            self.error(cause);
        }
    }

    fn class(&mut self, class: &Rc<Class>) {
        if !Self::first(&mut self.classes, class) {
            return;
        }
        let static_fields = class.static_fields.borrow();
        let static_values = static_fields.values().cloned().collect::<Vec<_>>();
        let static_field_bytes = static_fields.iter().fold(0_u64, |bytes, (key, _)| {
            bytes
                .saturating_add(LOGICAL_MAP_ENTRY_BYTES)
                .saturating_add(key.len() as u64)
        });
        drop(static_fields);
        let method_bytes = class
            .instance_methods
            .keys()
            .chain(class.static_methods.keys())
            .fold(0_u64, |bytes, key| {
                bytes
                    .saturating_add(LOGICAL_MAP_ENTRY_BYTES)
                    .saturating_add(key.len() as u64)
            });
        self.add(
            1,
            (class.name.len() as u64)
                .saturating_add(LOGICAL_REFERENCE_BYTES.saturating_mul(2))
                .saturating_add(method_bytes)
                .saturating_add(static_field_bytes),
        );
        if let Some(superclass) = &class.superclass {
            self.class(superclass);
        }
        if let Some(constructor) = &class.constructor {
            self.function(constructor);
        }
        for method in class
            .instance_methods
            .values()
            .chain(class.static_methods.values())
        {
            self.function(method);
        }
        for value in static_values {
            self.value(&value);
        }
    }

    fn instance(&mut self, instance: &Rc<Instance>) {
        if !Self::first(&mut self.instances, instance) {
            return;
        }
        let fields = instance.fields.borrow();
        let values = fields.values().cloned().collect::<Vec<_>>();
        let field_bytes = fields.iter().fold(0_u64, |bytes, (key, _)| {
            bytes
                .saturating_add(LOGICAL_MAP_ENTRY_BYTES)
                .saturating_add(key.len() as u64)
        });
        drop(fields);
        self.add(1, LOGICAL_REFERENCE_BYTES.saturating_add(field_bytes));
        self.class(&instance.class);
        for value in values {
            self.value(&value);
        }
    }

    fn function(&mut self, function: &Rc<Function>) {
        if !Self::first(&mut self.functions, function) {
            return;
        }
        self.add(1, LOGICAL_REFERENCE_BYTES);
        match &function.inner {
            FunctionKind::Bytecode { env, .. } => self.environment(env),
            FunctionKind::BoundMethod {
                function,
                receiver,
                context,
            } => {
                self.function(function);
                self.value(receiver);
                self.class(&context.owner);
            }
            FunctionKind::ReceiverBound {
                function,
                captured_receiver,
            } => {
                self.function(function);
                if let Some(receiver) = captured_receiver {
                    self.value(receiver);
                }
            }
            FunctionKind::Native { .. }
            | FunctionKind::ContextualNative { .. }
            | FunctionKind::ResourceBuiltin { .. }
            | FunctionKind::UnboundMethod { .. } => {}
        }
    }

    fn environment(&mut self, environment: &Env) {
        if BUILTIN_ENVIRONMENT.with(|builtins| Rc::ptr_eq(environment, builtins))
            || !Self::first(&mut self.environments, environment)
        {
            return;
        }
        self.add(1, 0);
        let environment = environment.borrow();
        let values = environment
            .slots
            .iter()
            .enumerate()
            .filter(|(slot, _)| environment.initialized.get(*slot).copied().unwrap_or(true))
            .map(|(_, (_, value))| value.clone())
            .collect::<Vec<_>>();
        let parent = environment.parent.clone();
        drop(environment);
        for value in values {
            self.value(&value);
        }
        if let Some(parent) = parent {
            self.environment(&parent);
        }
    }
}

struct RetainedMemoryTransaction {
    environments: Vec<(Env, EnvironmentSnapshot)>,
    classes: Vec<(Rc<Class>, BTreeMap<String, Value>)>,
    instances: Vec<(Rc<Instance>, BTreeMap<String, Value>)>,
    arrays: BTreeSet<usize>,
    maps: BTreeSet<usize>,
    errors: BTreeSet<usize>,
    classes_seen: BTreeSet<usize>,
    instances_seen: BTreeSet<usize>,
    functions: BTreeSet<usize>,
    environments_seen: BTreeSet<usize>,
}
impl RetainedMemoryTransaction {
    fn capture(root: &Env) -> Self {
        let mut transaction = Self {
            environments: vec![],
            classes: vec![],
            instances: vec![],
            arrays: BTreeSet::new(),
            maps: BTreeSet::new(),
            errors: BTreeSet::new(),
            classes_seen: BTreeSet::new(),
            instances_seen: BTreeSet::new(),
            functions: BTreeSet::new(),
            environments_seen: BTreeSet::new(),
        };
        transaction.environment(root);
        transaction
    }

    fn first<T: ?Sized>(seen: &mut BTreeSet<usize>, value: &Rc<T>) -> bool {
        seen.insert(RetainedMemoryCensus::rc_key(value))
    }

    fn value(&mut self, value: &Value) {
        match value {
            Value::Array(values) if Self::first(&mut self.arrays, values) => {
                for value in values.iter() {
                    self.value(value);
                }
            }
            Value::Map(values) if Self::first(&mut self.maps, values) => {
                for value in values.values() {
                    self.value(value);
                }
            }
            Value::Error(error) => self.error(error),
            Value::Class(class) => self.class(class),
            Value::Instance(instance) => self.instance(instance),
            Value::Function(function) => self.function(function),
            Value::Integer(_)
            | Value::Decimal(_)
            | Value::String(_)
            | Value::Array(_)
            | Value::Map(_)
            | Value::Nil
            | Value::Bool(_)
            | Value::Number(_) => {}
        }
    }

    fn error(&mut self, error: &Rc<ScriptError>) {
        if !Self::first(&mut self.errors, error) {
            return;
        }
        self.value(&error.data);
        if let Some(cause) = &error.cause {
            self.error(cause);
        }
    }

    fn class(&mut self, class: &Rc<Class>) {
        if !Self::first(&mut self.classes_seen, class) {
            return;
        }
        let fields = class.static_fields.borrow();
        let snapshot = fields.clone();
        let values = fields.values().cloned().collect::<Vec<_>>();
        drop(fields);
        self.classes.push((class.clone(), snapshot));
        if let Some(superclass) = &class.superclass {
            self.class(superclass);
        }
        if let Some(constructor) = &class.constructor {
            self.function(constructor);
        }
        for method in class
            .instance_methods
            .values()
            .chain(class.static_methods.values())
        {
            self.function(method);
        }
        for value in values {
            self.value(&value);
        }
    }

    fn instance(&mut self, instance: &Rc<Instance>) {
        if !Self::first(&mut self.instances_seen, instance) {
            return;
        }
        let fields = instance.fields.borrow();
        let snapshot = fields.clone();
        let values = fields.values().cloned().collect::<Vec<_>>();
        drop(fields);
        self.instances.push((instance.clone(), snapshot));
        self.class(&instance.class);
        for value in values {
            self.value(&value);
        }
    }

    fn function(&mut self, function: &Rc<Function>) {
        if !Self::first(&mut self.functions, function) {
            return;
        }
        match &function.inner {
            FunctionKind::Bytecode { env, .. } => self.environment(env),
            FunctionKind::BoundMethod {
                function,
                receiver,
                context,
            } => {
                self.function(function);
                self.value(receiver);
                self.class(&context.owner);
            }
            FunctionKind::ReceiverBound {
                function,
                captured_receiver,
            } => {
                self.function(function);
                if let Some(receiver) = captured_receiver {
                    self.value(receiver);
                }
            }
            FunctionKind::Native { .. }
            | FunctionKind::ContextualNative { .. }
            | FunctionKind::ResourceBuiltin { .. }
            | FunctionKind::UnboundMethod { .. } => {}
        }
    }

    fn environment(&mut self, environment: &Env) {
        if BUILTIN_ENVIRONMENT.with(|builtins| Rc::ptr_eq(environment, builtins))
            || !Self::first(&mut self.environments_seen, environment)
        {
            return;
        }
        let environment_ref = environment.borrow();
        let snapshot = environment_ref.snapshot();
        let values = environment_ref
            .slots
            .iter()
            .enumerate()
            .filter(|(slot, _)| {
                environment_ref
                    .initialized
                    .get(*slot)
                    .copied()
                    .unwrap_or(true)
            })
            .map(|(_, (_, value))| value.clone())
            .collect::<Vec<_>>();
        let parent = environment_ref.parent.clone();
        drop(environment_ref);
        self.environments.push((environment.clone(), snapshot));
        for value in values {
            self.value(&value);
        }
        if let Some(parent) = parent {
            self.environment(&parent);
        }
    }

    fn restore(self) {
        for (environment, snapshot) in self.environments {
            environment.borrow_mut().restore(snapshot);
        }
        for (class, snapshot) in self.classes {
            *class.static_fields.borrow_mut() = snapshot;
        }
        for (instance, snapshot) in self.instances {
            *instance.fields.borrow_mut() = snapshot;
        }
    }
}

fn retained_memory_limits_active(limits: ResourceLimits) -> bool {
    limits.max_retained_managed_objects() < u64::MAX
        || limits.max_retained_managed_bytes() < u64::MAX
}

fn check_retained_memory_limits(
    memory: RetainedMemory,
    limits: ResourceLimits,
) -> Result<(), Error> {
    if memory.objects > limits.max_retained_managed_objects() {
        return Err(Error::resource(
            ResourceLimit::RetainedManagedObjects,
            format!(
                "context retains {} managed objects, exceeding {}",
                memory.objects,
                limits.max_retained_managed_objects()
            ),
        ));
    }
    if memory.bytes > limits.max_retained_managed_bytes() {
        return Err(Error::resource(
            ResourceLimit::RetainedManagedBytes,
            format!(
                "context retains {} managed bytes, exceeding {}",
                memory.bytes,
                limits.max_retained_managed_bytes()
            ),
        ));
    }
    Ok(())
}

fn deep_managed_allocation(value: &Value) -> ManagedAllocation {
    let mut allocation = shallow_managed_allocation(value);
    match value {
        Value::Array(values) => {
            for child in values.iter() {
                allocation.add(deep_managed_allocation(child));
            }
        }
        Value::Map(values) => {
            for child in values.values() {
                allocation.add(deep_managed_allocation(child));
            }
        }
        _ => {}
    }
    allocation
}

fn managed_array_allocation(length: usize) -> ManagedAllocation {
    ManagedAllocation {
        legacy_value_allocations: 0,
        objects: 1,
        bytes: (length as u64).saturating_mul(LOGICAL_REFERENCE_BYTES),
    }
}

fn one_value_allocation(_: &[Value], value: &Value) -> ManagedAllocation {
    ManagedAllocation::legacy_shallow(1, value)
}

fn range_allocation(args: &[Value], value: &Value) -> ManagedAllocation {
    if matches!(args.first(), Some(Value::Integer(_))) {
        ManagedAllocation::legacy_deep(1, value)
    } else {
        ManagedAllocation::legacy_shallow(1, value)
    }
}

fn managed_value_allocation(_: &[Value], value: &Value) -> ManagedAllocation {
    ManagedAllocation::legacy_shallow(0, value)
}

fn integer_builtin_allocation(args: &[Value], value: &Value) -> ManagedAllocation {
    let mut allocation = if matches!(args.first(), Some(Value::Integer(_))) {
        ManagedAllocation::default()
    } else {
        shallow_managed_allocation(value)
    };
    allocation.legacy_value_allocations = 1;
    allocation
}

// Unicode White_Space, pinned explicitly so `trim` does not inherit locale or
// Unicode-table changes from the host Rust toolchain.
fn is_pinned_unicode_whitespace(character: char) -> bool {
    matches!(
        character,
        '\u{0009}'..='\u{000D}'
            | '\u{0020}'
            | '\u{0085}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
    )
}

fn array_and_element_allocations(_: &[Value], value: &Value) -> ManagedAllocation {
    match value {
        Value::Array(values) => ManagedAllocation::legacy_deep(values.len() as u64 + 1, value),
        _ => ManagedAllocation::default(),
    }
}

fn sorted_array_allocations(_: &[Value], value: &Value) -> ManagedAllocation {
    let legacy = match value {
        Value::Array(values) => values.len() as u64 + 1,
        _ => 0,
    };
    ManagedAllocation::legacy_shallow(legacy, value)
}

fn concat_allocations(_: &[Value], value: &Value) -> ManagedAllocation {
    let legacy = match value {
        Value::String(_) => 1,
        Value::Array(values) => values.len() as u64 + 1,
        _ => 0,
    };
    ManagedAllocation::legacy_shallow(legacy, value)
}

fn replace_all_allocations(_: &[Value], value: &Value) -> ManagedAllocation {
    ManagedAllocation::legacy_shallow(u64::from(matches!(value, Value::String(_))), value)
}

fn legacy_json_value_allocations(value: &Value) -> u64 {
    match value {
        Value::Integer(_) | Value::Decimal(_) | Value::String(_) => 1,
        Value::Array(values) => values
            .iter()
            .map(legacy_json_value_allocations)
            .fold(1_u64, u64::saturating_add),
        Value::Map(values) => values
            .values()
            .map(legacy_json_value_allocations)
            .fold(values.len() as u64 + 1, u64::saturating_add),
        Value::Nil
        | Value::Bool(_)
        | Value::Number(_)
        | Value::Error(_)
        | Value::Class(_)
        | Value::Instance(_)
        | Value::Function(_) => 0,
    }
}

fn json_value_allocations(_: &[Value], value: &Value) -> ManagedAllocation {
    ManagedAllocation::legacy_deep(legacy_json_value_allocations(value), value)
}

fn json_error(code: &'static str, failure: json::JsonFailure) -> Error {
    let limit = failure.resource_limit();
    let message = failure.to_string();
    match limit {
        Some(limit) => Error::resource(limit, message),
        None => Error::domain(code, message, Value::Nil),
    }
}

fn parse_json_builtin(xs: &[Value], limits: ResourceLimits) -> Result<Value, Error> {
    if xs.len() != 1 {
        return Err(Error::runtime("parse_json expects one string argument"));
    }
    let Value::String(source) = &xs[0] else {
        return Err(Error::runtime("parse_json expects a string"));
    };
    json::parse_json(source, limits).map_err(|error| json_error("json.parse", error))
}

fn encode_json_builtin(xs: &[Value], limits: ResourceLimits) -> Result<Value, Error> {
    if xs.len() != 1 {
        return Err(Error::runtime("encode_json expects one argument"));
    }
    json::encode_json(&xs[0], limits)
        .map(Value::from)
        .map_err(|error| json_error("json.encode", error))
}

fn sort_builtin(xs: &[Value], limits: ResourceLimits) -> Result<Value, Error> {
    if xs.len() != 1 {
        return Err(Error::runtime("sort expects one array argument"));
    }
    let Value::Array(input) = &xs[0] else {
        return Err(Error::runtime("sort expects an array"));
    };
    let item_limit = limits.max_collection_operation_items();
    if input.len() > item_limit {
        return Err(Error::resource(
            ResourceLimit::CollectionOperationItems,
            format!("sort input exceeds {item_limit} items"),
        ));
    }
    let Some(first) = input.first() else {
        return Ok(Value::Array(Rc::new(Vec::new())));
    };

    let sorted = match first {
        Value::Number(_) => {
            if !input
                .iter()
                .all(|value| matches!(value, Value::Number(number) if number.is_finite()))
            {
                return Err(Error::runtime(
                    "sort expects homogeneous finite numbers, integers, decimals, or strings",
                ));
            }
            let mut values = input.as_ref().clone();
            values.sort_by(|left, right| {
                let (Value::Number(left), Value::Number(right)) = (left, right) else {
                    unreachable!("sort validated homogeneous numbers")
                };
                left.partial_cmp(right)
                    .expect("sort validated finite numbers")
            });
            values
        }
        Value::Integer(_) => {
            if !input.iter().all(|value| matches!(value, Value::Integer(_))) {
                return Err(Error::runtime(
                    "sort expects homogeneous finite numbers, integers, decimals, or strings",
                ));
            }
            let mut values = input.as_ref().clone();
            values.sort_by(|left, right| {
                let (Value::Integer(left), Value::Integer(right)) = (left, right) else {
                    unreachable!("sort validated homogeneous integers")
                };
                left.cmp(right)
            });
            values
        }
        Value::Decimal(_) => {
            if !input.iter().all(|value| matches!(value, Value::Decimal(_))) {
                return Err(Error::runtime(
                    "sort expects homogeneous finite numbers, integers, decimals, or strings",
                ));
            }
            let scale = input
                .iter()
                .map(|value| match value {
                    Value::Decimal(value) => value.scale,
                    _ => unreachable!("sort validated homogeneous decimals"),
                })
                .max()
                .unwrap_or(0);
            let coefficient_limit = decimal_coefficient_bit_limit(limits);
            let mut keyed = Vec::with_capacity(input.len());
            for value in input.iter() {
                let Value::Decimal(decimal) = value else {
                    unreachable!("sort validated homogeneous decimals")
                };
                let growth = scale - decimal.scale;
                check_decimal_power_growth(decimal.inner(), growth, limits)?;
                let coefficient = decimal.inner() * decimal_power_of_ten(growth);
                if coefficient_limit < MAX_DECIMAL_BITS && coefficient.bits() > coefficient_limit {
                    return Err(Error::resource(
                        ResourceLimit::DecimalCoefficientBits,
                        format!(
                            "decimal aligned coefficient magnitude exceeds {coefficient_limit} bits"
                        ),
                    ));
                }
                keyed.push((value.clone(), coefficient));
            }
            keyed.sort_by(|left, right| left.1.cmp(&right.1));
            keyed.into_iter().map(|(value, _)| value).collect()
        }
        Value::String(_) => {
            if !input.iter().all(|value| matches!(value, Value::String(_))) {
                return Err(Error::runtime(
                    "sort expects homogeneous finite numbers, integers, decimals, or strings",
                ));
            }
            let mut values = input.as_ref().clone();
            values.sort_by(|left, right| {
                let (Value::String(left), Value::String(right)) = (left, right) else {
                    unreachable!("sort validated homogeneous strings")
                };
                left.cmp(right)
            });
            values
        }
        _ => {
            return Err(Error::runtime(
                "sort expects homogeneous finite numbers, integers, decimals, or strings",
            ));
        }
    };
    Ok(Value::Array(Rc::new(sorted)))
}

fn concat_builtin(xs: &[Value], limits: ResourceLimits) -> Result<Value, Error> {
    if xs.len() != 2 {
        return Err(Error::runtime("concat expects two arguments"));
    }
    match (&xs[0], &xs[1]) {
        (Value::String(left), Value::String(right)) => {
            let output_len = left.len().checked_add(right.len()).ok_or_else(|| {
                Error::resource(
                    ResourceLimit::StringBytes,
                    format!("string exceeds {} bytes", limits.max_string_bytes()),
                )
            })?;
            check_string_len_resource(output_len, limits)?;
            let mut output = String::with_capacity(output_len);
            output.push_str(left);
            output.push_str(right);
            Ok(Value::String(Rc::from(output)))
        }
        (Value::Array(left), Value::Array(right)) => {
            let output_len = left.len().checked_add(right.len()).ok_or_else(|| {
                Error::resource(
                    ResourceLimit::ArrayItems,
                    format!("array exceeds {} items", limits.max_array_items()),
                )
            })?;
            let operation_limit = limits.max_collection_operation_items();
            if output_len > operation_limit {
                return Err(Error::resource(
                    ResourceLimit::CollectionOperationItems,
                    format!("concat input exceeds {operation_limit} items"),
                ));
            }
            check_array_resource(output_len, limits)?;
            let mut output = Vec::with_capacity(output_len);
            output.extend(left.iter().cloned());
            output.extend(right.iter().cloned());
            Ok(Value::Array(Rc::new(output)))
        }
        _ => Err(Error::runtime(
            "concat expects two strings or two arrays of the same type",
        )),
    }
}

fn replace_all_builtin(xs: &[Value], limits: ResourceLimits) -> Result<Value, Error> {
    if xs.len() != 3 {
        return Err(Error::runtime("replace_all expects three string arguments"));
    }
    let (Value::String(input), Value::String(needle), Value::String(replacement)) =
        (&xs[0], &xs[1], &xs[2])
    else {
        return Err(Error::runtime("replace_all expects strings"));
    };
    if needle.is_empty() {
        return Err(Error::runtime("replace_all needle must not be empty"));
    }

    let operation_limit = limits.max_text_operation_bytes();
    if input.len() > operation_limit {
        return Err(Error::resource(
            ResourceLimit::TextOperationBytes,
            format!("replace_all input exceeds {operation_limit} UTF-8 bytes"),
        ));
    }

    let matches = input.match_indices(needle.as_ref()).count();
    let output_len = checked_replacement_output_len(
        input.len(),
        needle.len(),
        replacement.len(),
        matches,
        limits,
    )?;
    check_string_len_resource(output_len, limits)?;

    let mut output = String::with_capacity(output_len);
    let mut cursor = 0;
    for (index, matched) in input.match_indices(needle.as_ref()) {
        output.push_str(&input[cursor..index]);
        output.push_str(replacement);
        cursor = index + matched.len();
    }
    output.push_str(&input[cursor..]);
    Ok(Value::String(Rc::from(output)))
}

fn checked_replacement_output_len(
    input_len: usize,
    needle_len: usize,
    replacement_len: usize,
    matches: usize,
    limits: ResourceLimits,
) -> Result<usize, Error> {
    let output_len = if replacement_len >= needle_len {
        replacement_len
            .checked_sub(needle_len)
            .and_then(|growth| growth.checked_mul(matches))
            .and_then(|growth| input_len.checked_add(growth))
    } else {
        needle_len
            .checked_sub(replacement_len)
            .and_then(|shrink| shrink.checked_mul(matches))
            .and_then(|shrink| input_len.checked_sub(shrink))
    };
    output_len.ok_or_else(|| {
        Error::resource(
            ResourceLimit::StringBytes,
            format!("string exceeds {} bytes", limits.max_string_bytes()),
        )
    })
}

fn valid_error_code(code: &str) -> bool {
    code.len() <= 64
        && code
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && code.bytes().skip(1).all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn valid_error_data(value: &Value, depth: usize) -> bool {
    if depth > 64 {
        return false;
    }
    match value {
        Value::Nil | Value::Bool(_) | Value::Integer(_) | Value::Decimal(_) | Value::String(_) => {
            true
        }
        Value::Number(value) => value.is_finite(),
        Value::Array(values) => values
            .iter()
            .all(|value| valid_error_data(value, depth + 1)),
        Value::Map(values) => values
            .values()
            .all(|value| valid_error_data(value, depth + 1)),
        Value::Error(_) | Value::Class(_) | Value::Instance(_) | Value::Function(_) => false,
    }
}

fn error_cause_depth(error: &ScriptError) -> usize {
    1 + error.cause.as_deref().map_or(0, error_cause_depth)
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
impl Context {
    /// Creates a context with standard library builtins and the default fuel budget.
    pub fn new() -> Self {
        let global = BUILTIN_ENVIRONMENT.with(|builtins| env(Some(builtins.clone())));
        Self {
            engine: Engine::new(),
            runtime: None,
            global,
            fuel: 1_000_000,
            max_call_depth: 1_024,
            resource_limits: ResourceLimits::default(),
            cancellation: None,
            host_bindings: None,
            last_execution: ExecutionStats::default(),
            retained_memory_high_water: RetainedMemory {
                objects: 1,
                bytes: 0,
            },
        }
    }
    /// Starts configuration of a Context owned by a fresh default Runtime.
    ///
    /// Use [`Runtime::context_builder`] when multiple Contexts should share
    /// compilation artifacts.
    pub fn builder() -> ContextBuilder {
        Runtime::new().context_builder()
    }
    /// Returns a builder-style context with the supplied fuel budget.
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.set_fuel(fuel);
        self
    }
    /// Sets the instruction budget used by the next and subsequent runs.
    ///
    /// A context keeps its globals and registered native functions, so an
    /// embedding host can adjust a budget between runs without rebuilding it.
    pub fn set_fuel(&mut self, fuel: u64) {
        self.fuel = fuel;
    }
    /// Returns the instruction budget configured for each new run.
    pub fn fuel(&self) -> u64 {
        self.fuel
    }
    /// Returns this context with a maximum nested QuickCoffee function-call depth.
    ///
    /// A value of zero permits top-level code but rejects every bytecode function
    /// call. Native host callbacks do not add a QuickCoffee call frame.
    pub fn with_max_call_depth(mut self, max_call_depth: usize) -> Self {
        self.set_max_call_depth(max_call_depth);
        self
    }
    /// Sets the maximum nested QuickCoffee function-call depth for future runs.
    pub fn set_max_call_depth(&mut self, max_call_depth: usize) {
        self.max_call_depth = max_call_depth;
    }
    /// Returns the maximum nested QuickCoffee function-call depth for each run.
    pub fn max_call_depth(&self) -> usize {
        self.max_call_depth
    }
    /// Returns this context with the supplied deterministic data-size policy.
    pub fn with_resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.set_resource_limits(limits);
        self
    }
    /// Replaces deterministic data-size boundaries used by future operations.
    ///
    /// Existing globals and compiled programs remain valid. Module children and
    /// already-installed standard-library functions observe the replacement.
    pub fn set_resource_limits(&mut self, limits: ResourceLimits) {
        self.resource_limits = limits;
    }
    /// Returns the deterministic data-size policy used by future operations.
    pub fn resource_limits(&self) -> ResourceLimits {
        self.resource_limits
    }
    /// Returns this context configured to observe an embedding-host cancellation token.
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.set_cancellation_token(token);
        self
    }
    /// Sets or replaces the cancellation token observed by future runs.
    pub fn set_cancellation_token(&mut self, token: CancellationToken) {
        self.cancellation = Some(token);
    }
    /// Removes the configured cancellation token from future runs.
    pub fn clear_cancellation_token(&mut self) {
        self.cancellation = None;
    }
    fn host_bindings_mut(&mut self) -> &mut HostBindings {
        Rc::make_mut(
            self.host_bindings
                .get_or_insert_with(|| Rc::new(HostBindings::default())),
        )
    }
    fn compact_host_bindings(&mut self) {
        if self
            .host_bindings
            .as_deref()
            .is_some_and(HostBindings::is_empty)
        {
            self.host_bindings = None;
        }
    }
    /// Returns this Context with type-safe, script-invisible host state.
    pub fn with_host_state<T: 'static>(mut self, state: T) -> Self {
        self.set_host_state(state);
        self
    }
    /// Installs or replaces type-safe, script-invisible host state.
    pub fn set_host_state<T: 'static>(&mut self, state: T) {
        self.host_bindings_mut().state = Some(HostState::new(state));
    }
    /// Removes the current host state from future contextual native calls.
    pub fn clear_host_state(&mut self) {
        if let Some(bindings) = self.host_bindings.as_mut() {
            Rc::make_mut(bindings).state = None;
            self.compact_host_bindings();
        }
    }
    /// Returns the Context-owned host state when its concrete type matches `T`.
    pub fn host_state<T: 'static>(&self) -> Option<&T> {
        self.host_bindings.as_ref()?.state.as_ref()?.downcast_ref()
    }
    /// Replaces the typed capability allowlist used by future native calls.
    pub fn set_capabilities(&mut self, capabilities: HostCapabilities) {
        if capabilities.is_empty()
            && self
                .host_bindings
                .as_deref()
                .is_none_or(|bindings| bindings.state.is_none())
        {
            self.host_bindings = None;
        } else {
            self.host_bindings_mut().capabilities = capabilities;
        }
    }
    /// Returns a snapshot of this Context's capability allowlist.
    pub fn capabilities(&self) -> HostCapabilities {
        self.host_bindings
            .as_ref()
            .map_or_else(HostCapabilities::new, |bindings| {
                bindings.capabilities.clone()
            })
    }
    /// Installs or replaces one typed, script-invisible host capability.
    pub fn set_capability<T: 'static>(&mut self, key: CapabilityKey<T>, capability: T) {
        self.host_bindings_mut()
            .capabilities
            .insert(key, capability);
    }
    /// Returns this Context after installing one typed host capability.
    pub fn with_capability<T: 'static>(mut self, key: CapabilityKey<T>, capability: T) -> Self {
        self.set_capability(key, capability);
        self
    }
    /// Clones an allowlisted opaque capability when its slot and type match.
    pub fn capability<T: 'static>(&self, key: CapabilityKey<T>) -> Option<Rc<T>> {
        self.host_bindings.as_ref()?.capabilities.get(key)
    }
    /// Removes one capability only when its slot and concrete type match.
    pub fn remove_capability<T: 'static>(&mut self, key: CapabilityKey<T>) -> bool {
        let Some(bindings) = self.host_bindings.as_mut() else {
            return false;
        };
        let removed = Rc::make_mut(bindings).capabilities.remove(key);
        self.compact_host_bindings();
        removed
    }
    /// Removes every capability from future contextual native calls.
    pub fn clear_capabilities(&mut self) {
        if let Some(bindings) = self.host_bindings.as_mut() {
            Rc::make_mut(bindings).capabilities.clear();
            self.compact_host_bindings();
        }
    }
    /// Returns counters from the most recent successful or failed execution.
    /// Compilation and verification errors do not replace the previous record.
    pub fn last_execution(&self) -> ExecutionStats {
        self.last_execution
    }
    /// Returns a cycle-safe snapshot of QuickCoffee-managed values currently
    /// retained by this context.
    ///
    /// The shared standard-library parent and opaque host callback internals
    /// are excluded. Shared values and closure/environment cycles are counted
    /// once by identity; values held only by the embedding host are outside the
    /// context root and are not included.
    pub fn retained_memory(&self) -> RetainedMemory {
        let mut census = RetainedMemoryCensus::default();
        census.environment(&self.global);
        census.snapshot
    }
    /// Samples this context's currently retained managed-memory graph.
    ///
    /// The returned value has the same meaning as [`Context::retained_memory`].
    /// This explicit host operation also updates the component-wise lifetime
    /// high-water record returned by [`Context::retained_memory_high_water`].
    /// It deliberately does not run automatically after VM instructions or
    /// executions, so it is not a full live-memory peak and has no dispatch
    /// overhead when an embedding host does not request it.
    pub fn sample_retained_memory(&mut self) -> RetainedMemory {
        let snapshot = self.retained_memory();
        self.retained_memory_high_water.objects = self
            .retained_memory_high_water
            .objects
            .max(snapshot.objects);
        self.retained_memory_high_water.bytes =
            self.retained_memory_high_water.bytes.max(snapshot.bytes);
        snapshot
    }
    /// Returns the component-wise maximum of explicit retained-memory samples.
    ///
    /// A new context starts with its empty writable global environment sampled.
    /// This record lasts for the context lifetime and is not an allocator, RSS,
    /// or instruction-by-instruction live-memory peak.
    pub fn retained_memory_high_water(&self) -> RetainedMemory {
        self.retained_memory_high_water
    }
    /// Installs or replaces an immutable global value visible to later runs.
    pub fn set_global(&mut self, name: impl Into<String>, value: Value) {
        let name = name.into();
        self.global.borrow_mut().set_local(&name, value);
    }
    /// Returns this context after installing an immutable global value.
    ///
    /// This builder-style form is equivalent to [`Context::set_global`] and
    /// is convenient when configuring an embedding context inline.
    pub fn with_global(mut self, name: impl Into<String>, value: Value) -> Self {
        self.set_global(name, value);
        self
    }
    /// Reads a global value without exposing the VM environment or running code.
    pub fn get_global(&self, name: &str) -> Option<Value> {
        lookup(&self.global, name)
    }
    /// Registers a host callback as an opaque callable global.
    pub fn add_native<F>(&mut self, name: impl Into<String>, f: F)
    where
        F: Fn(&[Value]) -> Result<Value, Error> + 'static,
    {
        self.set_global(name, native_value(Rc::new(f)));
    }
    /// Registers a host callback with cooperative execution controls and typed state.
    pub fn add_contextual_native<F>(&mut self, name: impl Into<String>, function: F)
    where
        F: Fn(&mut NativeCallContext, &[Value]) -> Result<Value, Error> + 'static,
    {
        self.set_global(name, contextual_native_value(Rc::new(function)));
    }
    fn add_builtin<F>(
        &mut self,
        name: impl Into<String>,
        f: F,
        allocation_profile: fn(&[Value], &Value) -> ManagedAllocation,
    ) where
        F: Fn(&[Value]) -> Result<Value, Error> + 'static,
    {
        self.set_global(
            name,
            Value::Function(Rc::new(Function {
                inner: FunctionKind::Native {
                    function: Rc::new(f),
                    allocation_profile: Some(allocation_profile),
                },
            })),
        );
    }
    fn add_resource_builtin(
        &mut self,
        name: impl Into<String>,
        function: fn(&[Value], ResourceLimits) -> Result<Value, Error>,
        allocation_profile: fn(&[Value], &Value) -> ManagedAllocation,
    ) {
        self.set_global(
            name,
            Value::Function(Rc::new(Function {
                inner: FunctionKind::ResourceBuiltin {
                    function,
                    allocation_profile: Some(allocation_profile),
                },
            })),
        );
    }
    fn add_unprofiled_resource_builtin(
        &mut self,
        name: impl Into<String>,
        function: fn(&[Value], ResourceLimits) -> Result<Value, Error>,
    ) {
        self.set_global(
            name,
            Value::Function(Rc::new(Function {
                inner: FunctionKind::ResourceBuiltin {
                    function,
                    allocation_profile: None,
                },
            })),
        );
    }
    /// Returns this context after registering a host callback as a global.
    ///
    /// This builder-style form is equivalent to [`Context::add_native`].
    pub fn with_native<F>(mut self, name: impl Into<String>, f: F) -> Self
    where
        F: Fn(&[Value]) -> Result<Value, Error> + 'static,
    {
        self.add_native(name, f);
        self
    }
    /// Returns this Context after registering a contextual host callback.
    pub fn with_contextual_native<F>(mut self, name: impl Into<String>, function: F) -> Self
    where
        F: Fn(&mut NativeCallContext, &[Value]) -> Result<Value, Error> + 'static,
    {
        self.add_contextual_native(name, function);
        self
    }
    /// Compiles, verifies, and executes source in this context.
    pub fn eval(&mut self, source: &str) -> Result<Value, Error> {
        let program = match &self.runtime {
            Some(runtime) => runtime.compile_program(source)?,
            None => self.engine.compile_program(source)?,
        };
        self.run_program(&program)
    }
    /// Compiles, verifies, and executes source while attaching an opaque
    /// host-provided name to compile-time and runtime source labels. A name
    /// ending in `.litcoffee` enables literate CoffeeScript preprocessing.
    pub fn eval_named(&mut self, source_name: &str, source: &str) -> Result<Value, Error> {
        let program = match &self.runtime {
            Some(runtime) => runtime.compile_program_named(source_name, source)?,
            None => self.engine.compile_program_named(source_name, source)?,
        };
        self.run_program(&program)
    }
    /// Verifies and executes an owned bytecode chunk.
    pub fn run(&mut self, chunk: Chunk) -> Result<Value, Error> {
        self.run_program(&chunk.into())
    }
    /// Runs shared compiled bytecode without cloning its instruction stream.
    pub fn run_program(&mut self, program: &Program) -> Result<Value, Error> {
        program.ensure_verified()?;
        let retained_transaction = if retained_memory_limits_active(self.resource_limits) {
            check_retained_memory_limits(self.retained_memory(), self.resource_limits)?;
            Some(RetainedMemoryTransaction::capture(&self.global))
        } else {
            None
        };
        let mut vm = Vm {
            fuel: self.fuel,
            instructions: 0,
            max_call_depth: self.max_call_depth,
            resource_limits: self.resource_limits,
            value_limits_active: value_limits_active(self.resource_limits),
            call_depth: 0,
            call_depth_peak: 0,
            cancellation: self.cancellation.clone(),
            host_bindings: self
                .host_bindings
                .as_ref()
                .map(|bindings| bindings.clone() as HostBindingsViewHandle),
            name_loads: 0,
            name_stores: 0,
            calls: 0,
            container_ops: 0,
            iterator_ops: 0,
            exception_ops: 0,
            value_allocations: 0,
            environment_allocations: 0,
            managed_objects_allocated: 0,
            managed_bytes_allocated: 0,
            initial_debug_info: program.0.debug_info.clone(),
            execution_plan: program.0.execution_plan.clone(),
        };
        let result = vm.run(Rc::clone(&program.0.chunk), self.global.clone());
        self.last_execution = vm.stats();
        if let Some(transaction) = retained_transaction {
            if let Err(error) =
                check_retained_memory_limits(self.retained_memory(), self.resource_limits)
            {
                transaction.restore();
                return Err(error);
            }
        }
        result
    }
    pub(crate) fn module_child(&self) -> Self {
        Self {
            engine: self.engine.clone(),
            runtime: self.runtime.clone(),
            global: env(Some(self.global.clone())),
            fuel: self.fuel,
            max_call_depth: self.max_call_depth,
            resource_limits: self.resource_limits,
            cancellation: self.cancellation.clone(),
            host_bindings: self.host_bindings.clone(),
            last_execution: ExecutionStats::default(),
            retained_memory_high_water: RetainedMemory::default(),
        }
    }
    pub(crate) fn compile_module(&self, name: &str, source: &str) -> Result<Module, Error> {
        match &self.runtime {
            Some(runtime) => runtime.compile_module(name, source),
            None => self.engine.compile_module(name, source),
        }
    }
    pub(crate) fn get_local(&self, name: &str) -> Option<Value> {
        self.global.borrow().get_local(name)
    }
    pub(crate) fn set_execution_stats(&mut self, stats: ExecutionStats) {
        self.last_execution = stats;
    }
    fn install_builtins(&mut self) {
        self.add_native("print", |xs| {
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    print!(" ")
                }
                print!("{x}")
            }
            println!();
            Ok(Value::Nil)
        });
        self.add_native("len", |xs| {
            if xs.len() != 1 {
                return Err(Error::runtime("len expects one argument"));
            }
            let n = match &xs[0] {
                Value::String(x) => x.chars().count(),
                Value::Array(x) => x.len(),
                Value::Map(x) => x.len(),
                _ => return Err(Error::runtime("len expects string, array, or map")),
            };
            Ok(Value::Number(n as f64))
        });
        self.add_builtin(
            "type",
            |xs| {
                if xs.len() != 1 {
                    return Err(Error::runtime("type expects one argument"));
                }
                let n = match xs[0] {
                    Value::Nil => "nil",
                    Value::Bool(_) => "bool",
                    Value::Number(_) => "number",
                    Value::Integer(_) => "integer",
                    Value::Decimal(_) => "decimal",
                    Value::String(_) => "string",
                    Value::Array(_) => "array",
                    Value::Map(_) => "map",
                    Value::Error(_) => "error",
                    Value::Class(_) => "class",
                    Value::Instance(_) => "instance",
                    Value::Function(_) => "function",
                };
                Ok(Value::String(Rc::from(n)))
            },
            one_value_allocation,
        );
        self.add_resource_builtin(
            "range",
            |xs, limits| {
                if xs.len() != 2 {
                    return Err(Error::runtime("range expects two arguments"));
                }
                range_values(xs[0].clone(), xs[1].clone(), false, limits)
            },
            range_allocation,
        );
        self.add_builtin(
            "str",
            |xs| {
                if xs.len() != 1 {
                    return Err(Error::runtime("str expects one argument"));
                }
                Ok(Value::String(Rc::from(string_value(&xs[0]))))
            },
            one_value_allocation,
        );
        self.add_builtin(
            "trim",
            |xs| {
                if xs.len() != 1 {
                    return Err(Error::runtime("trim expects one string argument"));
                }
                let Value::String(input) = &xs[0] else {
                    return Err(Error::runtime("trim expects a string"));
                };
                Ok(Value::String(Rc::from(
                    input.trim_matches(is_pinned_unicode_whitespace),
                )))
            },
            one_value_allocation,
        );
        self.add_native("contains", |xs| {
            if xs.len() != 2 {
                return Err(Error::runtime("contains expects two string arguments"));
            }
            let (Value::String(input), Value::String(needle)) = (&xs[0], &xs[1]) else {
                return Err(Error::runtime("contains expects strings"));
            };
            Ok(Value::Bool(input.contains(needle.as_ref())))
        });
        self.add_native("starts_with", |xs| {
            if xs.len() != 2 {
                return Err(Error::runtime("starts_with expects two string arguments"));
            }
            let (Value::String(input), Value::String(prefix)) = (&xs[0], &xs[1]) else {
                return Err(Error::runtime("starts_with expects strings"));
            };
            Ok(Value::Bool(input.starts_with(prefix.as_ref())))
        });
        self.add_native("ends_with", |xs| {
            if xs.len() != 2 {
                return Err(Error::runtime("ends_with expects two string arguments"));
            }
            let (Value::String(input), Value::String(suffix)) = (&xs[0], &xs[1]) else {
                return Err(Error::runtime("ends_with expects strings"));
            };
            Ok(Value::Bool(input.ends_with(suffix.as_ref())))
        });
        self.add_resource_builtin("replace_all", replace_all_builtin, replace_all_allocations);
        self.add_resource_builtin("sort", sort_builtin, sorted_array_allocations);
        self.add_resource_builtin("concat", concat_builtin, concat_allocations);
        self.install_json_builtins();
        self.add_builtin(
            "integer",
            |xs| {
                if xs.len() != 1 {
                    return Err(Error::runtime("integer expects one argument"));
                }
                match &xs[0] {
                    Value::Integer(value) => Ok(Value::Integer(value.clone())),
                    Value::Decimal(value) if value.scale == 0 => Ok(Value::Integer(Rc::new(
                        Integer::from_bigint(value.inner().clone())?,
                    ))),
                    Value::Number(value) if value.is_finite() && value.fract() == 0. => {
                        let value = BigInt::from_f64(*value).ok_or_else(|| {
                            Error::runtime("number cannot be converted to integer")
                        })?;
                        Ok(Value::Integer(Rc::new(Integer::from_bigint(value)?)))
                    }
                    _ => Err(Error::runtime(
                        "integer expects an integer, whole decimal, or finite integral number",
                    )),
                }
            },
            integer_builtin_allocation,
        );
        self.add_unprofiled_resource_builtin("number", |xs, limits| {
            if xs.len() != 1 {
                return Err(Error::runtime("number expects one argument"));
            }
            match &xs[0] {
                Value::Number(value) => Ok(Value::Number(*value)),
                Value::Integer(value) => {
                    const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
                    let value = value
                        .as_i64()
                        .filter(|value| (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(value))
                        .ok_or_else(|| {
                            Error::runtime("integer is outside the safe number range")
                        })?;
                    Ok(Value::Number(value as f64))
                }
                Value::Decimal(value) => decimal_to_exact_number(value, limits).map(Value::Number),
                _ => Err(Error::runtime(
                    "number expects a number, integer, or exactly representable decimal",
                )),
            }
        });
        self.add_resource_builtin(
            "decimal",
            |xs, limits| {
                if xs.len() != 1 {
                    return Err(Error::runtime("decimal expects one argument"));
                }
                let value = match &xs[0] {
                    Value::Decimal(value) => (**value).clone(),
                    Value::Integer(value) => {
                        resource_decimal(value.inner().clone(), 0, limits)?
                    }
                    Value::String(value) => Decimal::parse_with_resource_limits(value, limits)?,
                    Value::Number(_) => {
                        return Err(Error::runtime(
                            "decimal does not accept Number; use a suffixed literal or decimal string",
                        ));
                    }
                    _ => {
                        return Err(Error::runtime(
                            "decimal expects a decimal, integer, or decimal string",
                        ));
                    }
                };
                Ok(Value::Decimal(Rc::new(value)))
            },
            one_value_allocation,
        );
        self.add_resource_builtin(
            "decimal_div",
            |xs, limits| {
                if xs.len() != 4 {
                    return Err(Error::runtime(
                        "decimal_div expects two decimals, scale, and rounding mode",
                    ));
                }
                let (Value::Decimal(left), Value::Decimal(right)) = (&xs[0], &xs[1]) else {
                    return Err(Error::runtime(
                        "decimal_div expects Decimal dividend and divisor",
                    ));
                };
                let scale = decimal_scale_argument(&xs[2], limits)?;
                let rounding = DecimalRounding::parse(&xs[3])?;
                Ok(Value::Decimal(Rc::new(decimal_div_rounded(
                    left, right, scale, rounding, limits,
                )?)))
            },
            one_value_allocation,
        );
        self.add_resource_builtin(
            "round_decimal",
            |xs, limits| {
                if xs.len() != 3 {
                    return Err(Error::runtime(
                        "round_decimal expects a decimal, scale, and rounding mode",
                    ));
                }
                let Value::Decimal(value) = &xs[0] else {
                    return Err(Error::runtime("round_decimal expects a Decimal value"));
                };
                let scale = decimal_scale_argument(&xs[1], limits)?;
                let rounding = DecimalRounding::parse(&xs[2])?;
                Ok(Value::Decimal(Rc::new(decimal_round(
                    value, scale, rounding, limits,
                )?)))
            },
            one_value_allocation,
        );
        self.add_builtin(
            "abs",
            |xs| {
                if xs.len() != 1 {
                    return Err(Error::runtime("abs expects one number"));
                }
                match &xs[0] {
                    Value::Number(value) if value.is_finite() => Ok(Value::Number(value.abs())),
                    Value::Integer(value) => Ok(Value::Integer(Rc::new(Integer::from_bigint(
                        value.inner().abs(),
                    )?))),
                    Value::Decimal(value) => Ok(Value::Decimal(Rc::new(Decimal::from_bigint(
                        value.inner().abs(),
                        value.scale,
                    )?))),
                    _ => Err(Error::runtime(
                        "abs expects a finite number, integer, or decimal",
                    )),
                }
            },
            managed_value_allocation,
        );
        self.add_resource_builtin(
            "sum",
            |xs, limits| aggregate_numeric(xs, "sum", Aggregate::Sum, limits),
            managed_value_allocation,
        );
        self.add_resource_builtin(
            "min",
            |xs, limits| aggregate_numeric(xs, "min", Aggregate::Min, limits),
            managed_value_allocation,
        );
        self.add_resource_builtin(
            "max",
            |xs, limits| aggregate_numeric(xs, "max", Aggregate::Max, limits),
            managed_value_allocation,
        );
        self.add_builtin(
            "error",
            |xs| {
                if !(2..=4).contains(&xs.len()) {
                    return Err(Error::runtime(
                        "error expects code, message, optional data, and optional cause",
                    ));
                }
                let Value::String(code) = &xs[0] else {
                    return Err(Error::runtime("error code must be string"));
                };
                if !valid_error_code(code) {
                    return Err(Error::runtime("invalid error code"));
                }
                let Value::String(message) = &xs[1] else {
                    return Err(Error::runtime("error message must be string"));
                };
                let data = xs.get(2).cloned().unwrap_or(Value::Nil);
                if !valid_error_data(&data, 0) {
                    return Err(Error::runtime(
                        "error data must be finite immutable business data",
                    ));
                }
                let cause = match xs.get(3) {
                    None | Some(Value::Nil) => None,
                    Some(Value::Error(cause)) => Some(cause.clone()),
                    Some(_) => return Err(Error::runtime("error cause must be error or nil")),
                };
                if cause
                    .as_deref()
                    .is_some_and(|cause| error_cause_depth(cause) >= 64)
                {
                    return Err(Error::runtime("error cause chain is too deep"));
                }
                Ok(Value::Error(Rc::new(ScriptError {
                    code: code.clone(),
                    message: message.clone(),
                    data,
                    cause,
                    trusted_labels: Vec::new(),
                })))
            },
            one_value_allocation,
        );
        self.add_resource_builtin(
            "keys",
            |xs, limits| {
                if xs.len() != 1 {
                    return Err(Error::runtime("keys expects one argument"));
                }
                let Value::Map(map) = &xs[0] else {
                    return Err(Error::runtime("keys expects a map"));
                };
                check_array_resource(map.len(), limits)?;
                Ok(Value::Array(Rc::new(
                    map.keys()
                        .map(|key| Value::String(Rc::from(key.as_str())))
                        .collect(),
                )))
            },
            array_and_element_allocations,
        );
        self.add_resource_builtin(
            "values",
            |xs, limits| {
                if xs.len() != 1 {
                    return Err(Error::runtime("values expects one argument"));
                }
                let Value::Map(map) = &xs[0] else {
                    return Err(Error::runtime("values expects a map"));
                };
                check_array_resource(map.len(), limits)?;
                Ok(Value::Array(Rc::new(map.values().cloned().collect())))
            },
            one_value_allocation,
        );
        self.add_resource_builtin(
            "join",
            |xs, limits| {
                if xs.len() != 2 {
                    return Err(Error::runtime("join expects array and separator"));
                }
                let Value::Array(values) = &xs[0] else {
                    return Err(Error::runtime("join expects an array"));
                };
                let Value::String(separator) = &xs[1] else {
                    return Err(Error::runtime("join separator must be string"));
                };
                let mut output = String::new();
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        let length =
                            output.len().checked_add(separator.len()).ok_or_else(|| {
                                Error::resource(ResourceLimit::StringBytes, "string is too large")
                            })?;
                        check_string_len_resource(length, limits)?;
                        output.push_str(separator);
                    }
                    let value = string_value(value);
                    let length = output.len().checked_add(value.len()).ok_or_else(|| {
                        Error::resource(ResourceLimit::StringBytes, "string is too large")
                    })?;
                    check_string_len_resource(length, limits)?;
                    output.push_str(&value);
                }
                Ok(Value::String(Rc::from(output)))
            },
            one_value_allocation,
        );
        self.add_resource_builtin(
            "split",
            |xs, limits| {
                if xs.len() != 2 {
                    return Err(Error::runtime("split expects string and separator"));
                }
                let Value::String(input) = &xs[0] else {
                    return Err(Error::runtime("split expects a string"));
                };
                let Value::String(separator) = &xs[1] else {
                    return Err(Error::runtime("split separator must be string"));
                };
                check_array_resource(input.split(separator.as_ref()).count(), limits)?;
                let parts: Vec<_> = input
                    .split(separator.as_ref())
                    .map(|part| Value::String(Rc::from(part)))
                    .collect();
                Ok(Value::Array(Rc::new(parts)))
            },
            array_and_element_allocations,
        );
        self.add_native("assert", |xs| {
            let Some(Value::Bool(condition)) = xs.first() else {
                return Err(Error::runtime("assert expects a boolean condition"));
            };
            if *condition {
                return Ok(Value::Nil);
            }
            let detail = match xs.get(1) {
                Some(Value::String(message)) => format!(": {message}"),
                Some(_) => return Err(Error::runtime("assert message must be string")),
                None => String::new(),
            };
            Err(Error::runtime(format!("assertion failed{detail}")))
        });
    }

    fn install_json_builtins(&mut self) {
        self.add_resource_builtin("parse_json", parse_json_builtin, json_value_allocations);
        self.add_resource_builtin("encode_json", encode_json_builtin, one_value_allocation);
    }
}
struct Frame {
    chunk: Rc<Chunk>,
    pc: usize,
    stack: Vec<Value>,
    iterators: Vec<Iteration>,
    handlers: Vec<Handler>,
    debug_info: Option<Rc<ProgramDebugInfo>>,
    execution_plan: Option<Rc<ProgramExecutionPlan>>,
    env: Env,
    bindings: FrameBindings,
    receiver: Option<Value>,
    method_context: Option<MethodContext>,
    receiver_initialized: bool,
    super_called: bool,
    initialize_caller_receiver_on_return: bool,
    return_override: Option<Value>,
}
enum FrameBindings {
    Raw,
    Guarded(Rc<ChunkBindingSlots>),
    Fast {
        slots: Rc<ChunkBindingSlots>,
        locals: Vec<Option<Value>>,
    },
    Shared(Rc<ChunkBindingSlots>),
}
fn lookup_environment<const SHARED: bool>(
    environment: &Env,
    cached: Option<&Cell<Option<usize>>>,
    name: &str,
) -> Option<Value> {
    let current = environment;
    let environment = current.borrow();
    if let Some(slot) = cached.and_then(Cell::get) {
        let value = if SHARED {
            environment.get_shared_cached(name, slot)
        } else {
            environment.get_cached(name, slot)
        };
        if let Some(value) = value {
            return Some(value);
        }
    }
    if let Some((slot, value)) = environment.get_local_with_slot(name) {
        if let Some(cached) = cached {
            cached.set(Some(slot));
        }
        return Some(value);
    }
    let parent = environment.parent.clone();
    drop(environment);
    parent.and_then(|parent| lookup_parent_environment(current, parent, cached, name))
}
#[cold]
#[inline(never)]
fn lookup_parent_environment(
    current: &Env,
    parent: Env,
    cached: Option<&Cell<Option<usize>>>,
    name: &str,
) -> Option<Value> {
    let is_builtin_parent = BUILTIN_ENVIRONMENT.with(|builtins| Rc::ptr_eq(&parent, builtins));
    if is_builtin_parent {
        let value = parent.borrow().get_local(name)?;
        let slot = current.borrow_mut().set_local(name, value.clone());
        if let Some(cached) = cached {
            cached.set(Some(slot));
        }
        return Some(value);
    }
    lookup(&parent, name)
}
fn store_environment<const SHARED: bool>(
    environment: &Env,
    cached: Option<&Cell<Option<usize>>>,
    name: &str,
    value: Value,
) {
    let mut environment = environment.borrow_mut();
    if let Some(slot) = cached.and_then(Cell::get) {
        let result = if SHARED {
            environment.set_shared_cached(name, slot, value)
        } else {
            environment.set_cached(name, slot, value)
        };
        match result {
            Ok(()) => return,
            Err(value) => {
                let slot = environment.set_local(name, value);
                if let Some(cached) = cached {
                    cached.set(Some(slot));
                }
                return;
            }
        }
    }
    let slot = environment.set_local(name, value);
    if let Some(cached) = cached {
        cached.set(Some(slot));
    }
}
fn lookup_frame(frame: &Frame, pc: usize, name: &str) -> Option<Value> {
    match &frame.bindings {
        FrameBindings::Fast { slots, locals } => {
            if let Some(slot) = slots.local_by_pc.get(pc).copied().flatten() {
                if let Some(value) = locals.get(slot)?.clone() {
                    return Some(value);
                }
                let parent = frame.env.borrow().parent.clone();
                return parent.and_then(|parent| lookup(&parent, name));
            }
            // An elided fast frame may point directly at a shared captured
            // environment whose preallocated slot is still unset.
            lookup_environment::<true>(&frame.env, slots.cached_by_pc.get(pc), name)
        }
        FrameBindings::Guarded(slots) => {
            lookup_environment::<false>(&frame.env, slots.cached_by_pc.get(pc), name)
        }
        FrameBindings::Shared(slots) => {
            if let Some(slot) = slots.local_by_pc.get(pc).copied().flatten() {
                let environment = frame.env.borrow();
                match environment.get_resolved(slot) {
                    Some(Some(value)) => return Some(value),
                    Some(None) => {
                        let parent = environment.parent.clone();
                        drop(environment);
                        return parent.and_then(|parent| lookup(&parent, name));
                    }
                    None => drop(environment),
                }
            }
            lookup_environment::<true>(&frame.env, slots.cached_by_pc.get(pc), name)
        }
        FrameBindings::Raw => lookup_environment::<false>(&frame.env, None, name),
    }
}
fn store_frame(frame: &mut Frame, pc: usize, name: &str, value: Value) {
    match &mut frame.bindings {
        FrameBindings::Fast { slots, locals } => {
            if let Some(slot) = slots.local_by_pc.get(pc).copied().flatten() {
                locals[slot] = Some(value);
                return;
            }
            store_environment::<true>(&frame.env, slots.cached_by_pc.get(pc), name, value);
        }
        FrameBindings::Guarded(slots) => {
            store_environment::<false>(&frame.env, slots.cached_by_pc.get(pc), name, value);
        }
        FrameBindings::Shared(slots) => {
            if let Some(slot) = slots.local_by_pc.get(pc).copied().flatten() {
                if let Err(value) = frame.env.borrow_mut().set_resolved(slot, value) {
                    store_environment::<true>(&frame.env, slots.cached_by_pc.get(pc), name, value);
                }
                return;
            }
            store_environment::<true>(&frame.env, slots.cached_by_pc.get(pc), name, value);
        }
        FrameBindings::Raw => store_environment::<false>(&frame.env, None, name, value),
    }
}
struct Handler {
    catch_pc: usize,
    stack_depth: usize,
    iterator_depth: usize,
    name: String,
}
struct Iteration {
    kind: IterationKind,
}
enum IterationKind {
    Array {
        values: Rc<Vec<Value>>,
        position: usize,
        step: i64,
    },
    String {
        values: Rc<Vec<Value>>,
        position: usize,
        step: i64,
    },
    Map {
        entries: Vec<(String, Value)>,
        position: usize,
    },
}
struct Vm {
    fuel: u64,
    instructions: u64,
    max_call_depth: usize,
    resource_limits: ResourceLimits,
    value_limits_active: bool,
    call_depth: usize,
    call_depth_peak: usize,
    cancellation: Option<CancellationToken>,
    host_bindings: Option<HostBindingsViewHandle>,
    name_loads: u64,
    name_stores: u64,
    calls: u64,
    container_ops: u64,
    iterator_ops: u64,
    exception_ops: u64,
    value_allocations: u64,
    environment_allocations: u64,
    managed_objects_allocated: u64,
    managed_bytes_allocated: u64,
    initial_debug_info: Option<Rc<ProgramDebugInfo>>,
    execution_plan: Option<Rc<ProgramExecutionPlan>>,
}
enum Step {
    Continue,
    Return(Value),
    Call {
        target: CallTarget,
        args: Vec<Value>,
        return_override: Option<Value>,
        initialize_caller_receiver: bool,
    },
}
enum CallTarget {
    Value(Value),
    Receiver {
        function: Rc<Function>,
        receiver: Value,
        context: MethodContext,
    },
}
impl Vm {
    fn source_span(frame: &Frame, pc: usize) -> Option<SourceSpan> {
        frame
            .debug_info
            .as_ref()
            .and_then(|debug_info| debug_info.span(&frame.chunk, pc))
    }

    fn with_call_stack(mut error: Error, frames: &[Frame], include_current_call: bool) -> Error {
        let skip = usize::from(!include_current_call);
        for frame in frames.iter().rev().skip(skip) {
            error = error.with_secondary_span(Self::source_span(frame, frame.pc.saturating_sub(1)));
        }
        error
    }

    fn record_value_allocations(&mut self, count: u64) {
        self.value_allocations = self.value_allocations.saturating_add(count);
    }

    fn record_managed_allocation(&mut self, allocation: ManagedAllocation) {
        self.value_allocations = self
            .value_allocations
            .saturating_add(allocation.legacy_value_allocations);
        self.managed_objects_allocated = self
            .managed_objects_allocated
            .saturating_add(allocation.objects);
        self.managed_bytes_allocated = self
            .managed_bytes_allocated
            .saturating_add(allocation.bytes);
    }

    fn record_shallow_value_allocation(&mut self, legacy: u64, value: &Value) {
        self.record_managed_allocation(ManagedAllocation::legacy_shallow(legacy, value));
    }

    fn record_deep_value_allocation(&mut self, legacy: u64, value: &Value) {
        self.record_managed_allocation(ManagedAllocation::legacy_deep(legacy, value));
    }

    fn record_stack_managed_allocation(&mut self, frame: &Frame) {
        self.record_managed_allocation(shallow_managed_allocation(
            frame.stack.last().expect("managed numeric result"),
        ));
    }

    fn record_environment_allocation(&mut self) {
        self.environment_allocations = self.environment_allocations.saturating_add(1);
        self.managed_objects_allocated = self.managed_objects_allocated.saturating_add(1);
    }

    // This boundary keeps argument-buffer plumbing out of the monolithic
    // instruction dispatch loop, where code layout affects unrelated workloads.
    #[inline(never)]
    fn direct_call(frame: &mut Frame, argument_count: usize) -> Result<Step, Error> {
        if frame.stack.len() < argument_count {
            return Err(Error::runtime("stack underflow"));
        }
        let mut args =
            REUSABLE_CALL_ARGUMENTS.with(|reusable| std::mem::take(&mut *reusable.borrow_mut()));
        args.clear();
        let argument_start = frame.stack.len() - argument_count;
        args.extend(frame.stack.drain(argument_start..));
        let callee = match pop(frame) {
            Ok(callee) => callee,
            Err(error) => {
                Self::recycle_call_arguments(args);
                return Err(error);
            }
        };
        Ok(Step::Call {
            target: CallTarget::Value(callee),
            args,
            return_override: None,
            initialize_caller_receiver: false,
        })
    }

    #[inline(never)]
    fn direct_member_call(
        &mut self,
        frame: &mut Frame,
        argument_count: usize,
        name: &str,
    ) -> Result<Step, Error> {
        if frame.stack.len() < argument_count + 1 {
            return Err(Error::runtime("stack underflow"));
        }
        let mut args =
            REUSABLE_CALL_ARGUMENTS.with(|reusable| std::mem::take(&mut *reusable.borrow_mut()));
        args.clear();
        let argument_start = frame.stack.len() - argument_count;
        args.extend(frame.stack.drain(argument_start..));
        let target = match pop(frame) {
            Ok(target) => target,
            Err(error) => {
                Self::recycle_call_arguments(args);
                return Err(error);
            }
        };
        let (target, preserves_profile_allocation) = match member_call_target(target, name) {
            Ok(target) => target,
            Err(error) => {
                Self::recycle_call_arguments(args);
                return Err(error);
            }
        };
        if preserves_profile_allocation {
            self.record_value_allocations(1);
        }
        Ok(Step::Call {
            target,
            args,
            return_override: None,
            initialize_caller_receiver: false,
        })
    }

    fn construct(
        &mut self,
        frame: &mut Frame,
        class: Value,
        args: Vec<Value>,
    ) -> Result<Step, Error> {
        let Value::Class(class) = class else {
            return Err(Error::runtime("new expects a QuickCoffee class"));
        };
        let instance = Value::Instance(Rc::new(Instance {
            class: class.clone(),
            fields: RefCell::new(BTreeMap::new()),
        }));
        self.record_shallow_value_allocation(1, &instance);
        if let Some((owner, constructor)) = find_constructor(&class) {
            return Ok(Step::Call {
                target: CallTarget::Receiver {
                    function: constructor,
                    receiver: instance.clone(),
                    context: MethodContext {
                        owner,
                        name: Rc::from("constructor"),
                        kind: MethodKind::Constructor,
                    },
                },
                args,
                return_override: Some(instance),
                initialize_caller_receiver: false,
            });
        }
        if !args.is_empty() {
            return Err(Error::runtime(format!(
                "class {} has no constructor and expects no arguments",
                class.name
            )));
        }
        frame.stack.push(instance);
        Ok(Step::Continue)
    }

    #[inline(never)]
    fn super_call(&mut self, frame: &mut Frame, args: Vec<Value>) -> Result<Step, Error> {
        let context = frame
            .method_context
            .clone()
            .ok_or_else(|| Error::runtime("super call outside class member"))?;
        let receiver = frame
            .receiver
            .clone()
            .ok_or_else(|| Error::runtime("super call has no receiver"))?;
        let parent = context.owner.superclass.clone().ok_or_else(|| {
            Error::runtime(format!("class {} has no parent class", context.owner.name))
        })?;
        match context.kind {
            MethodKind::Constructor => {
                if frame.super_called {
                    return Err(Error::runtime(
                        "derived constructor cannot call super more than once",
                    ));
                }
                frame.super_called = true;
                let Some((owner, constructor)) = find_constructor(&parent) else {
                    if !args.is_empty() {
                        return Err(Error::runtime(format!(
                            "class {} has no constructor and expects no arguments",
                            parent.name
                        )));
                    }
                    frame.receiver_initialized = true;
                    frame.stack.push(Value::Nil);
                    return Ok(Step::Continue);
                };
                Ok(Step::Call {
                    target: CallTarget::Receiver {
                        function: constructor,
                        receiver,
                        context: MethodContext {
                            owner,
                            name: Rc::from("constructor"),
                            kind: MethodKind::Constructor,
                        },
                    },
                    args,
                    return_override: Some(Value::Nil),
                    initialize_caller_receiver: true,
                })
            }
            MethodKind::Instance => {
                let (owner, function) =
                    find_instance_method(&parent, &context.name).ok_or_else(|| {
                        Error::runtime(format!(
                            "parent of {} has no instance method '{}'",
                            context.owner.name, context.name
                        ))
                    })?;
                Ok(Step::Call {
                    target: CallTarget::Receiver {
                        function,
                        receiver,
                        context: MethodContext {
                            owner,
                            name: context.name,
                            kind: MethodKind::Instance,
                        },
                    },
                    args,
                    return_override: None,
                    initialize_caller_receiver: false,
                })
            }
            MethodKind::Static => {
                let (owner, function) =
                    find_static_method(&parent, &context.name).ok_or_else(|| {
                        Error::runtime(format!(
                            "parent of {} has no static method '{}'",
                            context.owner.name, context.name
                        ))
                    })?;
                Ok(Step::Call {
                    target: CallTarget::Receiver {
                        function,
                        receiver,
                        context: MethodContext {
                            owner,
                            name: context.name,
                            kind: MethodKind::Static,
                        },
                    },
                    args,
                    return_override: None,
                    initialize_caller_receiver: false,
                })
            }
        }
    }

    #[inline(never)]
    fn make_class(
        &mut self,
        frame: &mut Frame,
        name: &str,
        extends: bool,
        has_constructor: bool,
        instance_methods: &[String],
        static_methods: &[String],
    ) -> Result<(), Error> {
        let count = usize::from(has_constructor) + instance_methods.len() + static_methods.len();
        let mut functions = take(frame, count)?.into_iter();
        let superclass = if extends {
            let parent = pop(frame)?;
            let Value::Class(parent) = parent else {
                return Err(Error::runtime(format!(
                    "class {name} extends value must be a QuickCoffee class"
                )));
            };
            let mut seen = BTreeSet::new();
            let mut current = Some(parent.clone());
            while let Some(class) = current {
                if !seen.insert(Rc::as_ptr(&class) as usize) {
                    return Err(Error::runtime(
                        "parent class chain contains an inheritance cycle",
                    ));
                }
                current = class.superclass.clone();
            }
            Some(parent)
        } else {
            None
        };
        let constructor = if has_constructor {
            Some(value_function(
                functions.next().expect("verified constructor count"),
                "constructor",
            )?)
        } else {
            None
        };
        let mut instance_table = BTreeMap::new();
        for method in instance_methods {
            instance_table.insert(
                method.clone(),
                value_function(
                    functions.next().expect("verified instance method count"),
                    method,
                )?,
            );
        }
        let mut static_table = BTreeMap::new();
        for method in static_methods {
            static_table.insert(
                method.clone(),
                value_function(
                    functions.next().expect("verified static method count"),
                    method,
                )?,
            );
        }
        if let Some(parent) = &superclass {
            for (method, function) in &instance_table {
                if function_uses_super(function) && find_instance_method(parent, method).is_none() {
                    return Err(Error::runtime(format!(
                        "class {name} method '{method}' uses super but does not override a parent instance method"
                    )));
                }
            }
            for (method, function) in &static_table {
                if function_uses_super(function) && find_static_method(parent, method).is_none() {
                    return Err(Error::runtime(format!(
                        "class {name} static method '{method}' uses super but does not override a parent static method"
                    )));
                }
            }
        }
        let class = Value::Class(Rc::new(Class {
            name: Rc::from(name),
            superclass,
            constructor,
            instance_methods: instance_table,
            static_methods: static_table,
            static_fields: RefCell::new(BTreeMap::new()),
        }));
        self.record_shallow_value_allocation(1, &class);
        frame.stack.push(class);
        Ok(())
    }

    fn recycle_call_arguments(mut args: Vec<Value>) {
        args.clear();
        REUSABLE_CALL_ARGUMENTS.with(|reusable| {
            let mut reusable = reusable.borrow_mut();
            if args.capacity() > MAX_REUSABLE_CALL_ARGUMENTS {
                if reusable.capacity() == 0 {
                    *reusable = Vec::with_capacity(MAX_REUSABLE_CALL_ARGUMENTS);
                }
            } else if args.capacity() > reusable.capacity() {
                *reusable = args;
            }
        });
    }

    fn take_frame_stack() -> Vec<Value> {
        REUSABLE_FRAME_STACK.with(|reusable| std::mem::take(&mut *reusable.borrow_mut()))
    }

    fn recycle_frame_stack(mut stack: Vec<Value>) {
        stack.clear();
        REUSABLE_FRAME_STACK.with(|reusable| {
            let mut reusable = reusable.borrow_mut();
            if stack.capacity() > MAX_REUSABLE_FRAME_STACK {
                if reusable.capacity() == 0 {
                    *reusable = Vec::with_capacity(MAX_REUSABLE_FRAME_STACK);
                }
            } else if stack.capacity() > reusable.capacity() {
                *reusable = stack;
            }
        });
    }

    fn record_profile(&mut self, instruction: &Instruction) {
        match instruction {
            Instruction::Load(_) | Instruction::LoadOrNil(_) => self.name_loads += 1,
            Instruction::Store(_) => self.name_stores += 1,
            Instruction::Call(_)
            | Instruction::CallSpread
            | Instruction::MemberCall { .. }
            | Instruction::MemberCallSpread(_)
            | Instruction::SuperCall(_)
            | Instruction::SuperCallSpread
            | Instruction::Construct(_)
            | Instruction::ConstructSpread => self.calls += 1,
            Instruction::MakeArray(_)
            | Instruction::Append
            | Instruction::MergeArrays(_)
            | Instruction::MergeMaps(_)
            | Instruction::MakeRange(_)
            | Instruction::MakeMap(_)
            | Instruction::Index
            | Instruction::Slice(_)
            | Instruction::Member(_)
            | Instruction::SetMember(_)
            | Instruction::MakeClass { .. }
            | Instruction::Contains
            | Instruction::HasKey => self.container_ops += 1,
            Instruction::IterStartEnumerable
            | Instruction::IterStartMap
            | Instruction::IterNext { .. }
            | Instruction::IterEnd => self.iterator_ops += 1,
            Instruction::Try { .. } | Instruction::EndTry | Instruction::Throw => {
                self.exception_ops += 1
            }
            _ => {}
        }
    }
    fn eval_default(
        &mut self,
        chunk: Rc<Chunk>,
        env: Env,
        debug_info: Option<Rc<ProgramDebugInfo>>,
        execution_plan: Option<Rc<ProgramExecutionPlan>>,
    ) -> Result<Value, Error> {
        let mut nested = Vm {
            fuel: self.fuel,
            instructions: self.instructions,
            max_call_depth: self.max_call_depth,
            resource_limits: self.resource_limits,
            value_limits_active: self.value_limits_active,
            call_depth: self.call_depth,
            call_depth_peak: self.call_depth_peak,
            cancellation: self.cancellation.clone(),
            host_bindings: self.host_bindings.clone(),
            name_loads: self.name_loads,
            name_stores: self.name_stores,
            calls: self.calls,
            container_ops: self.container_ops,
            iterator_ops: self.iterator_ops,
            exception_ops: self.exception_ops,
            value_allocations: self.value_allocations,
            environment_allocations: self.environment_allocations,
            managed_objects_allocated: self.managed_objects_allocated,
            managed_bytes_allocated: self.managed_bytes_allocated,
            initial_debug_info: debug_info,
            execution_plan,
        };
        let result = nested.run(chunk, env);
        self.fuel = nested.fuel;
        self.instructions = nested.instructions;
        self.call_depth = nested.call_depth;
        self.call_depth_peak = nested.call_depth_peak;
        self.name_loads = nested.name_loads;
        self.name_stores = nested.name_stores;
        self.calls = nested.calls;
        self.container_ops = nested.container_ops;
        self.iterator_ops = nested.iterator_ops;
        self.exception_ops = nested.exception_ops;
        self.value_allocations = nested.value_allocations;
        self.environment_allocations = nested.environment_allocations;
        self.managed_objects_allocated = nested.managed_objects_allocated;
        self.managed_bytes_allocated = nested.managed_bytes_allocated;
        result
    }
    fn stats(&self) -> ExecutionStats {
        ExecutionStats {
            instructions: self.instructions,
            fuel_remaining: self.fuel,
            call_depth_peak: self.call_depth_peak,
            name_loads: self.name_loads,
            name_stores: self.name_stores,
            calls: self.calls,
            container_ops: self.container_ops,
            iterator_ops: self.iterator_ops,
            exception_ops: self.exception_ops,
            value_allocations: self.value_allocations,
            environment_allocations: self.environment_allocations,
            managed_objects_allocated: self.managed_objects_allocated,
            managed_bytes_allocated: self.managed_bytes_allocated,
        }
    }
    fn run(&mut self, chunk: Rc<Chunk>, global: Env) -> Result<Value, Error> {
        let execution_plan = self.execution_plan.clone();
        let binding_slots = execution_plan.as_ref().and_then(|plan| plan.slots(&chunk));
        let mut frames = vec![Frame {
            chunk,
            pc: 0,
            stack: vec![],
            iterators: vec![],
            handlers: vec![],
            debug_info: self.initial_debug_info.clone(),
            execution_plan,
            env: global,
            bindings: binding_slots
                .map(FrameBindings::Guarded)
                .unwrap_or(FrameBindings::Raw),
            receiver: None,
            method_context: None,
            receiver_initialized: true,
            super_called: false,
            initialize_caller_receiver_on_return: false,
            return_override: None,
        }];
        loop {
            if self
                .cancellation
                .as_ref()
                .is_some_and(CancellationToken::is_cancelled)
            {
                let span = frames
                    .last()
                    .and_then(|frame| Self::source_span(frame, frame.pc));
                let error =
                    Error::resource(ResourceLimit::Cancellation, "execution cancelled by host")
                        .with_span_if_missing(span);
                return Err(Self::with_call_stack(error, &frames, false));
            }
            if self.fuel == 0 {
                let span = frames
                    .last()
                    .and_then(|frame| Self::source_span(frame, frame.pc));
                let error = Error::resource(ResourceLimit::Fuel, "execution fuel exhausted")
                    .with_span_if_missing(span);
                return Err(Self::with_call_stack(error, &frames, false));
            }
            self.fuel -= 1;
            self.instructions += 1;
            let step = (|| -> Result<Step, Error> {
                let frame = frames.last_mut().expect("VM has an initial frame");
                let instruction_pc = frame.pc;
                let chunk = frame.chunk.clone();
                let op = chunk
                    .code
                    .get(instruction_pc)
                    .ok_or_else(|| Error::runtime("instruction pointer escaped chunk"))?;
                frame.pc += 1;
                self.record_profile(op);
                match op {
                    Instruction::Constant(i) => match frame
                        .chunk
                        .constants
                        .get(*i)
                        .ok_or_else(|| Error::runtime("invalid constant"))?
                    {
                        Constant::Value(v) => {
                            if self.value_limits_active && value_needs_resource_check(v) {
                                check_value_resources(v, self.resource_limits)?;
                            }
                            frame.stack.push(v.clone())
                        }
                        _ => {
                            return Err(Error::runtime("function template used as value constant"));
                        }
                    },
                    Instruction::Load(n) => {
                        let value = lookup_frame(frame, instruction_pc, n)
                            .ok_or_else(|| Error::runtime(format!("unknown name '{n}'")))?;
                        if self.value_limits_active && value_needs_resource_check(&value) {
                            check_value_resources(&value, self.resource_limits)?;
                        }
                        frame.stack.push(value);
                    }
                    Instruction::LoadOrNil(n) => {
                        let value = lookup_frame(frame, instruction_pc, n).unwrap_or(Value::Nil);
                        if self.value_limits_active && value_needs_resource_check(&value) {
                            check_value_resources(&value, self.resource_limits)?;
                        }
                        frame.stack.push(value);
                    }
                    Instruction::LoadReceiver => {
                        if !frame.receiver_initialized {
                            return Err(Error::runtime(
                                "derived constructor cannot access its receiver before super",
                            ));
                        }
                        let receiver = frame
                            .receiver
                            .clone()
                            .ok_or_else(|| Error::runtime("receiver load outside class member"))?;
                        frame.stack.push(receiver);
                    }
                    Instruction::Store(n) => {
                        let v = pop(frame)?;
                        store_frame(frame, instruction_pc, n, v.clone());
                        frame.stack.push(v)
                    }
                    Instruction::Destructure(pattern) => {
                        let value = pop(frame)?;
                        let env = frame.env.clone();
                        let (bindings, allocations) = if static_pattern_matches(pattern, &value) {
                            collect_static_pattern_bindings(pattern, &value)
                        } else {
                            let mut bindings = vec![];
                            let snapshot = env.borrow().snapshot();
                            if let Err(error) = bind_pattern(
                                self,
                                pattern,
                                Some(&value),
                                &mut bindings,
                                &env,
                                frame.debug_info.as_ref(),
                                frame.execution_plan.as_ref(),
                            ) {
                                env.borrow_mut().restore(snapshot);
                                return Err(error);
                            }
                            (bindings, ManagedAllocation::default())
                        };
                        self.record_managed_allocation(allocations);
                        let mut environment = frame.env.borrow_mut();
                        for (name, item) in bindings {
                            if name != "_" {
                                environment.set_local(&name, item);
                            }
                        }
                        drop(environment);
                        frame.stack.push(value);
                    }
                    Instruction::Pop => {
                        pop(frame)?;
                    }
                    Instruction::Dup => {
                        let value = frame
                            .stack
                            .last()
                            .cloned()
                            .ok_or_else(|| Error::runtime("stack underflow"))?;
                        frame.stack.push(value);
                    }
                    Instruction::Swap => {
                        if frame.stack.len() < 2 {
                            return Err(Error::runtime("stack underflow"));
                        }
                        let last = frame.stack.len() - 1;
                        frame.stack.swap(last, last - 1);
                    }
                    Instruction::Rotate3 => {
                        if frame.stack.len() < 3 {
                            return Err(Error::runtime("stack underflow"));
                        }
                        let start = frame.stack.len() - 3;
                        frame.stack[start..].rotate_left(1);
                    }
                    Instruction::Neg => match pop(frame)? {
                        Value::Number(x) => frame.stack.push(Value::Number(-x)),
                        Value::Integer(x) => {
                            push_integer(frame, -x.inner(), self.resource_limits)?;
                            self.record_managed_allocation(shallow_managed_allocation(
                                frame.stack.last().expect("integer result"),
                            ));
                        }
                        Value::Decimal(x) => {
                            let value = Value::Decimal(Rc::new(resource_decimal(
                                -x.inner(),
                                x.scale,
                                self.resource_limits,
                            )?));
                            self.record_managed_allocation(shallow_managed_allocation(&value));
                            frame.stack.push(value);
                        }
                        _ => {
                            return Err(Error::runtime(
                                "unary '-' expects number, integer, or decimal",
                            ));
                        }
                    },
                    Instruction::Not => {
                        let x = truth(pop(frame)?)?;
                        frame.stack.push(Value::Bool(!x))
                    }
                    Instruction::BitNot => match pop(frame)? {
                        Value::Number(x) => {
                            let x = bit_integer(Value::Number(x))?;
                            frame.stack.push(Value::Number((!x) as f64));
                        }
                        Value::Integer(x) => {
                            push_integer(frame, !x.inner(), self.resource_limits)?;
                            self.record_managed_allocation(shallow_managed_allocation(
                                frame.stack.last().expect("integer result"),
                            ));
                        }
                        _ => return Err(Error::runtime("'~' expects number or integer")),
                    },
                    Instruction::Exists => {
                        let value = pop(frame)?;
                        frame.stack.push(Value::Bool(!matches!(value, Value::Nil)))
                    }
                    Instruction::Increment => {
                        if numeric_update(frame, true, &self.resource_limits)? {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::Decrement => {
                        if numeric_update(frame, false, &self.resource_limits)? {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::Add => {
                        if numeric_binary(
                            frame,
                            &self.resource_limits,
                            |a, b| a + b,
                            |a, b, _| Ok(a + b),
                            decimal_add,
                        )? {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::Sub => {
                        if numeric_binary(
                            frame,
                            &self.resource_limits,
                            |a, b| a - b,
                            |a, b, _| Ok(a - b),
                            decimal_sub,
                        )? {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::Mul => {
                        if numeric_binary(
                            frame,
                            &self.resource_limits,
                            |a, b| a * b,
                            integer_mul_resource,
                            decimal_mul,
                        )? {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::Div => {
                        if numeric_binary(
                            frame,
                            &self.resource_limits,
                            |a, b| a / b,
                            |a, b, _| integer_div(a, b),
                            decimal_exact_div,
                        )? {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::FloorDiv => {
                        if numeric_binary(
                            frame,
                            &self.resource_limits,
                            |a, b| (a / b).floor(),
                            |a, b, _| integer_floor_div(a, b),
                            decimal_floor_div,
                        )? {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::Rem => {
                        if numeric_binary(
                            frame,
                            &self.resource_limits,
                            |a, b| a % b,
                            |a, b, _| integer_rem(a, b),
                            decimal_rem,
                        )? {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::Modulo => {
                        if numeric_binary(
                            frame,
                            &self.resource_limits,
                            |a, b| (a % b + b) % b,
                            |a, b, _| integer_modulo(a, b),
                            decimal_modulo,
                        )? {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::BitAnd => {
                        let managed = matches!(frame.stack.last(), Some(Value::Integer(_)));
                        numeric_bit_binary(
                            frame,
                            &self.resource_limits,
                            |a, b| a & b,
                            |a, b| a & b,
                        )?;
                        if managed {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::BitOr => {
                        let managed = matches!(frame.stack.last(), Some(Value::Integer(_)));
                        numeric_bit_binary(
                            frame,
                            &self.resource_limits,
                            |a, b| a | b,
                            |a, b| a | b,
                        )?;
                        if managed {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::BitXor => {
                        let managed = matches!(frame.stack.last(), Some(Value::Integer(_)));
                        numeric_bit_binary(
                            frame,
                            &self.resource_limits,
                            |a, b| a ^ b,
                            |a, b| a ^ b,
                        )?;
                        if managed {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::ShiftLeft => {
                        let managed = matches!(frame.stack.last(), Some(Value::Integer(_)));
                        numeric_shift(frame, false, &self.resource_limits)?;
                        if managed {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::ShiftRight => {
                        let managed = matches!(frame.stack.last(), Some(Value::Integer(_)));
                        numeric_shift(frame, true, &self.resource_limits)?;
                        if managed {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::ShiftRightUnsigned => {
                        bit_shift(frame, |a, b| ((a as u32).wrapping_shr(b)) as i32)?
                    }
                    Instruction::Pow => {
                        if numeric_binary(
                            frame,
                            &self.resource_limits,
                            |a, b| a.powf(b),
                            integer_pow,
                            decimal_pow,
                        )? {
                            self.record_stack_managed_allocation(frame);
                        }
                    }
                    Instruction::Eq => compare(frame, equal)?,
                    Instruction::Ne => compare(frame, |a, b| !equal(a, b))?,
                    Instruction::Lt => numeric_order(
                        frame,
                        &self.resource_limits,
                        |a, b| a < b,
                        |a, b| a < b,
                        |ordering| ordering.is_lt(),
                    )?,
                    Instruction::Le => numeric_order(
                        frame,
                        &self.resource_limits,
                        |a, b| a <= b,
                        |a, b| a <= b,
                        |ordering| ordering.is_le(),
                    )?,
                    Instruction::Gt => numeric_order(
                        frame,
                        &self.resource_limits,
                        |a, b| a > b,
                        |a, b| a > b,
                        |ordering| ordering.is_gt(),
                    )?,
                    Instruction::Ge => numeric_order(
                        frame,
                        &self.resource_limits,
                        |a, b| a >= b,
                        |a, b| a >= b,
                        |ordering| ordering.is_ge(),
                    )?,
                    Instruction::Contains => {
                        let target = pop(frame)?;
                        let needle = pop(frame)?;
                        let Value::Array(values) = target else {
                            return Err(Error::runtime("right side of 'in' must be an array"));
                        };
                        frame.stack.push(Value::Bool(
                            values.iter().any(|value| equal(&needle, value)),
                        ));
                    }
                    Instruction::HasKey => {
                        let target = pop(frame)?;
                        let key = pop(frame)?;
                        let (Value::String(key), Value::Map(values)) = (key, target) else {
                            return Err(Error::runtime("'of' expects a string key and a map"));
                        };
                        frame
                            .stack
                            .push(Value::Bool(values.contains_key(key.as_ref())));
                    }
                    Instruction::Jump(delta) => jump(frame, *delta)?,
                    Instruction::JumpIfFalse(delta) => {
                        if !truth(
                            frame
                                .stack
                                .last()
                                .cloned()
                                .ok_or_else(|| Error::runtime("stack underflow"))?,
                        )? {
                            jump(frame, *delta)?
                        }
                    }
                    Instruction::JumpIfNil(delta) => {
                        if matches!(frame.stack.last(), Some(Value::Nil)) {
                            jump(frame, *delta)?
                        }
                    }
                    Instruction::Try { catch, name } => frame.handlers.push(Handler {
                        catch_pc: (frame.pc as i64 + *catch as i64)
                            .try_into()
                            .map_err(|_| Error::runtime("invalid catch target"))?,
                        stack_depth: frame.stack.len(),
                        iterator_depth: frame.iterators.len(),
                        name: name.clone(),
                    }),
                    Instruction::EndTry => {
                        frame
                            .handlers
                            .pop()
                            .ok_or_else(|| Error::runtime("handler stack underflow"))?;
                    }
                    Instruction::Throw => {
                        let value = pop(frame)?;
                        return Err(match value {
                            Value::Error(error) => Error::from_script_error(error),
                            value => {
                                let message = format!("thrown: {value}");
                                let script_error = Rc::new(ScriptError {
                                    code: Rc::from("throw"),
                                    message: Rc::from(message.as_str()),
                                    data: value,
                                    cause: None,
                                    trusted_labels: Vec::new(),
                                });
                                self.record_managed_allocation(shallow_managed_allocation(
                                    &Value::Error(script_error.clone()),
                                ));
                                Error::from_script_error(script_error)
                            }
                        });
                    }
                    Instruction::IterStartEnumerable => {
                        let step = array_iteration_step(pop(frame)?)?;
                        match pop(frame)? {
                            Value::Array(values) => {
                                // Negative steps traverse from the final element so the
                                // optional index remains the actual array position.
                                let position = if step < 0 {
                                    values.len().saturating_sub(1)
                                } else {
                                    0
                                };
                                frame.iterators.push(Iteration {
                                    kind: IterationKind::Array {
                                        values,
                                        position,
                                        step,
                                    },
                                });
                            }
                            Value::String(value) => {
                                let values: Vec<_> = value
                                    .chars()
                                    .map(|character| Value::String(Rc::from(character.to_string())))
                                    .collect();
                                let character_count = values.len();
                                self.record_managed_allocation(ManagedAllocation {
                                    legacy_value_allocations: (character_count as u64)
                                        .saturating_add(1),
                                    objects: (character_count as u64).saturating_add(1),
                                    bytes: (character_count as u64)
                                        .saturating_mul(LOGICAL_REFERENCE_BYTES)
                                        .saturating_add(value.len() as u64),
                                });
                                frame.iterators.push(Iteration {
                                    kind: IterationKind::String {
                                        values: Rc::new(values),
                                        position: if step < 0 {
                                            character_count.saturating_sub(1)
                                        } else {
                                            0
                                        },
                                        step,
                                    },
                                });
                            }
                            _ => {
                                return Err(Error::runtime(
                                    "for expects an array or string iterable",
                                ));
                            }
                        }
                    }
                    Instruction::IterStartMap => match pop(frame)? {
                        Value::Map(map) => frame.iterators.push(Iteration {
                            kind: IterationKind::Map {
                                entries: map
                                    .iter()
                                    .map(|(key, value)| (key.clone(), value.clone()))
                                    .collect(),
                                position: 0,
                            },
                        }),
                        _ => return Err(Error::runtime("for of expects a map iterable")),
                    },
                    Instruction::IterNext { patterns, end } => {
                        let next = {
                            let iterator = frame
                                .iterators
                                .last_mut()
                                .ok_or_else(|| Error::runtime("iterator stack underflow"))?;
                            match &mut iterator.kind {
                                IterationKind::Array {
                                    values,
                                    position,
                                    step,
                                } => {
                                    let value = values.get(*position).cloned().map(|value| {
                                        if patterns.len() == 2 {
                                            vec![value, Value::Number(*position as f64)]
                                        } else {
                                            vec![value]
                                        }
                                    });
                                    if value.is_some() {
                                        advance_position(position, *step);
                                    }
                                    value
                                }
                                IterationKind::String {
                                    values,
                                    position,
                                    step,
                                } => {
                                    let value = values.get(*position).cloned().map(|value| {
                                        if patterns.len() == 2 {
                                            vec![value, Value::Number(*position as f64)]
                                        } else {
                                            vec![value]
                                        }
                                    });
                                    if value.is_some() {
                                        advance_position(position, *step);
                                    }
                                    value
                                }
                                IterationKind::Map { entries, position } => {
                                    let value = entries.get(*position).map(|(key, value)| {
                                        vec![Value::String(Rc::from(key.as_str())), value.clone()]
                                    });
                                    if let Some(values) = &value {
                                        self.record_shallow_value_allocation(1, &values[0]);
                                        *position += 1;
                                    }
                                    value
                                }
                            }
                        };
                        if let Some(values) = next {
                            if patterns.len() != values.len() {
                                return Err(Error::runtime("iterator binding arity mismatch"));
                            }
                            if patterns.iter().all(|pattern| {
                                matches!(pattern, Pattern::Bind(_) | Pattern::Ignore)
                            }) {
                                let mut environment = frame.env.borrow_mut();
                                for (pattern, value) in patterns.iter().zip(values) {
                                    if let Pattern::Bind(name) = pattern {
                                        environment.set_local(name, value);
                                    }
                                }
                            } else if patterns
                                .iter()
                                .zip(values.iter())
                                .all(|(pattern, value)| static_pattern_matches(pattern, value))
                            {
                                let mut bindings = vec![];
                                let mut allocations = ManagedAllocation::default();
                                for (pattern, value) in patterns.iter().zip(values.iter()) {
                                    let (mut pattern_bindings, pattern_allocations) =
                                        collect_static_pattern_bindings(pattern, value);
                                    bindings.append(&mut pattern_bindings);
                                    allocations.add(pattern_allocations);
                                }
                                self.record_managed_allocation(allocations);
                                let mut environment = frame.env.borrow_mut();
                                for (name, value) in bindings {
                                    environment.set_local(&name, value);
                                }
                            } else {
                                let mut bindings = vec![];
                                let snapshot = frame.env.borrow().snapshot();
                                for (pattern, value) in patterns.iter().zip(values.iter()) {
                                    let env = frame.env.clone();
                                    if let Err(error) = bind_pattern(
                                        self,
                                        pattern,
                                        Some(value),
                                        &mut bindings,
                                        &env,
                                        frame.debug_info.as_ref(),
                                        frame.execution_plan.as_ref(),
                                    ) {
                                        frame.env.borrow_mut().restore(snapshot);
                                        return Err(error);
                                    }
                                }
                                let mut environment = frame.env.borrow_mut();
                                for (name, value) in bindings {
                                    environment.set_local(&name, value);
                                }
                            }
                        } else {
                            frame.iterators.pop();
                            jump(frame, *end)?;
                        }
                    }
                    Instruction::IterEnd => {
                        frame
                            .iterators
                            .pop()
                            .ok_or_else(|| Error::runtime("iterator stack underflow"))?;
                    }
                    Instruction::MakeArray(n) => {
                        check_array_resource(*n, self.resource_limits)?;
                        let v = take(frame, *n)?;
                        let value = Value::Array(Rc::new(v));
                        self.record_shallow_value_allocation(1, &value);
                        frame.stack.push(value);
                    }
                    Instruction::Append => {
                        let value = pop(frame)?;
                        let Value::Array(mut values) = pop(frame)? else {
                            return Err(Error::runtime("append expects an array"));
                        };
                        let next_len = values.len().checked_add(1).ok_or_else(|| {
                            Error::resource(ResourceLimit::ArrayItems, "array is too large")
                        })?;
                        check_array_resource(next_len, self.resource_limits)?;
                        let cloned_backing = Rc::strong_count(&values) > 1;
                        Rc::make_mut(&mut values).push(value);
                        if cloned_backing {
                            let mut allocation = managed_array_allocation(values.len());
                            allocation.legacy_value_allocations = 1;
                            self.record_managed_allocation(allocation);
                        }
                        frame.stack.push(Value::Array(values));
                    }
                    Instruction::MergeArrays(n) => {
                        let segments = take(frame, *n)?;
                        let mut total = 0usize;
                        for segment in &segments {
                            let Value::Array(segment) = segment else {
                                return Err(Error::runtime("splat expects an array"));
                            };
                            total = total.checked_add(segment.len()).ok_or_else(|| {
                                Error::resource(ResourceLimit::ArrayItems, "array is too large")
                            })?;
                        }
                        check_array_resource(total, self.resource_limits)?;
                        let mut values = vec![];
                        for segment in segments {
                            let Value::Array(segment) = segment else {
                                return Err(Error::runtime("splat expects an array"));
                            };
                            values.extend(segment.iter().cloned());
                        }
                        let value = Value::Array(Rc::new(values));
                        self.record_shallow_value_allocation(1, &value);
                        frame.stack.push(value);
                    }
                    Instruction::MergeMaps(n) => {
                        let segments = take(frame, *n)?;
                        let mut values = BTreeMap::new();
                        for segment in segments {
                            let Value::Map(segment) = segment else {
                                return Err(Error::runtime("map spread expects a map"));
                            };
                            values.extend(
                                segment
                                    .iter()
                                    .map(|(key, value)| (key.clone(), value.clone())),
                            );
                        }
                        check_map_resource(values.len(), self.resource_limits)?;
                        let value = Value::Map(Rc::new(values));
                        self.record_shallow_value_allocation(1, &value);
                        frame.stack.push(value);
                    }
                    Instruction::MakeRange(inclusive) => {
                        let end = pop(frame)?;
                        let start = pop(frame)?;
                        let exact = matches!(start, Value::Integer(_));
                        let value = range_values(start, end, *inclusive, self.resource_limits)?;
                        if exact {
                            self.record_deep_value_allocation(1, &value);
                        } else {
                            self.record_shallow_value_allocation(1, &value);
                        }
                        frame.stack.push(value);
                    }
                    Instruction::MakeMap(keys) => {
                        check_map_resource(keys.len(), self.resource_limits)?;
                        for key in keys {
                            check_string_resource(key, self.resource_limits)?;
                        }
                        let v = take(frame, keys.len())?;
                        let value = Value::Map(Rc::new(keys.iter().cloned().zip(v).collect()));
                        self.record_shallow_value_allocation(1, &value);
                        frame.stack.push(value);
                    }
                    Instruction::Stringify => {
                        let value = pop(frame)?;
                        let value = string_value(&value);
                        check_string_resource(&value, self.resource_limits)?;
                        let value = Value::String(Rc::from(value));
                        self.record_shallow_value_allocation(1, &value);
                        frame.stack.push(value);
                    }
                    Instruction::Concat(n) => {
                        let values = take(frame, *n)?;
                        let mut output = String::new();
                        for value in values {
                            let Value::String(value) = value else {
                                return Err(Error::runtime("concat received non-string"));
                            };
                            let length =
                                output.len().checked_add(value.len()).ok_or_else(|| {
                                    Error::resource(
                                        ResourceLimit::StringBytes,
                                        "string is too large",
                                    )
                                })?;
                            check_string_len_resource(length, self.resource_limits)?;
                            output.push_str(&value);
                        }
                        let value = Value::String(Rc::from(output));
                        self.record_shallow_value_allocation(1, &value);
                        frame.stack.push(value);
                    }
                    Instruction::Index => {
                        let key = pop(frame)?;
                        let target = pop(frame)?;
                        frame.stack.push(index(self, target, key)?)
                    }
                    Instruction::Slice(inclusive) => {
                        let end = pop(frame)?;
                        let start = pop(frame)?;
                        let target = pop(frame)?;
                        frame.stack.push(slice(
                            self,
                            target,
                            start,
                            end,
                            *inclusive,
                            self.resource_limits,
                        )?)
                    }
                    Instruction::Member(name) => {
                        let target = pop(frame)?;
                        let value = member_value(target, name, false)?;
                        check_member_value_resources(&value, self.resource_limits)?;
                        if matches!(value, Value::Function(_)) {
                            self.record_shallow_value_allocation(1, &value);
                        }
                        frame.stack.push(value);
                    }
                    Instruction::SetMember(name) => {
                        if !frame.receiver_initialized {
                            return Err(Error::runtime(
                                "derived constructor cannot access its receiver before super",
                            ));
                        }
                        let value = pop(frame)?;
                        let target = frame.receiver.clone().ok_or_else(|| {
                            Error::runtime("receiver field write outside class member")
                        })?;
                        if set_receiver_member(target, name, value.clone(), self.resource_limits)? {
                            self.record_managed_allocation(ManagedAllocation {
                                legacy_value_allocations: 0,
                                objects: 0,
                                bytes: LOGICAL_MAP_ENTRY_BYTES.saturating_add(name.len() as u64),
                            });
                        }
                        frame.stack.push(value);
                    }
                    Instruction::MemberCall { name, count } => {
                        return self.direct_member_call(frame, *count, name);
                    }
                    Instruction::MemberCallSpread(name) => {
                        let args = pop(frame)?;
                        let Value::Array(args) = args else {
                            return Err(Error::runtime("splat call expects an array"));
                        };
                        let target = pop(frame)?;
                        let (target, preserves_profile_allocation) =
                            member_call_target(target, name)?;
                        if preserves_profile_allocation {
                            self.record_value_allocations(1);
                        }
                        return Ok(Step::Call {
                            target,
                            args: args.as_ref().clone(),
                            return_override: None,
                            initialize_caller_receiver: false,
                        });
                    }
                    Instruction::SuperCall(count) => {
                        let args = take(frame, *count)?;
                        return self.super_call(frame, args);
                    }
                    Instruction::SuperCallSpread => {
                        let args = pop(frame)?;
                        let Value::Array(args) = args else {
                            return Err(Error::runtime("super splat expects an array"));
                        };
                        return self.super_call(frame, args.as_ref().clone());
                    }
                    Instruction::MakeFunction(i) | Instruction::MakeBoundFunction(i) => match frame
                        .chunk
                        .constants
                        .get(*i)
                        .ok_or_else(|| Error::runtime("invalid function template"))?
                    {
                        Constant::Function {
                            params,
                            required,
                            rest,
                            receiver,
                            receiver_bound,
                            chunk,
                        } => {
                            let captured_receiver = if matches!(
                                op,
                                Instruction::MakeBoundFunction(_)
                            ) {
                                if !frame.receiver_initialized {
                                    return Err(Error::runtime(
                                        "derived constructor cannot capture its receiver before super",
                                    ));
                                }
                                Some(frame.receiver.clone().ok_or_else(|| {
                                    Error::runtime(
                                        "bound function creation outside class receiver context",
                                    )
                                })?)
                            } else {
                                None
                            };
                            let debug_info = frame.debug_info.clone();
                            let fast_parameters = frame
                                .execution_plan
                                .as_ref()
                                .and_then(|plan| plan.slots(chunk))
                                .and_then(|slots| {
                                    slots.fast_parameter_slots(params, *required, rest.as_deref())
                                });
                            let function = Rc::new(Function {
                                inner: FunctionKind::Bytecode {
                                    params: params.clone(),
                                    required: *required,
                                    rest: rest.clone(),
                                    receiver: *receiver,
                                    chunk: chunk.clone(),
                                    debug_info,
                                    execution_plan: frame.execution_plan.clone(),
                                    fast_parameters,
                                    env: frame.env.clone(),
                                },
                            });
                            let function = if *receiver_bound {
                                Rc::new(Function {
                                    inner: FunctionKind::ReceiverBound {
                                        function,
                                        captured_receiver,
                                    },
                                })
                            } else {
                                debug_assert!(captured_receiver.is_none());
                                function
                            };
                            let value = Value::Function(function);
                            let mut allocation = ManagedAllocation::legacy_shallow(1, &value);
                            if *receiver_bound {
                                allocation.objects = allocation.objects.saturating_add(1);
                                allocation.bytes =
                                    allocation.bytes.saturating_add(LOGICAL_REFERENCE_BYTES);
                            }
                            self.record_managed_allocation(allocation);
                            frame.stack.push(value);
                        }
                        _ => return Err(Error::runtime("value used as function template")),
                    },
                    Instruction::MakeClass {
                        name,
                        extends,
                        constructor,
                        instance_methods,
                        static_methods,
                    } => {
                        self.make_class(
                            frame,
                            name,
                            *extends,
                            *constructor,
                            instance_methods,
                            static_methods,
                        )?;
                    }
                    Instruction::Call(n) => {
                        return Self::direct_call(frame, *n);
                    }
                    Instruction::CallSpread => {
                        let args = pop(frame)?;
                        let Value::Array(args) = args else {
                            return Err(Error::runtime("splat call expects an array"));
                        };
                        let callee = pop(frame)?;
                        return Ok(Step::Call {
                            target: CallTarget::Value(callee),
                            args: args.as_ref().clone(),
                            return_override: None,
                            initialize_caller_receiver: false,
                        });
                    }
                    Instruction::Construct(count) => {
                        let args = take(frame, *count)?;
                        let class = pop(frame)?;
                        return self.construct(frame, class, args);
                    }
                    Instruction::ConstructSpread => {
                        let args = pop(frame)?;
                        let Value::Array(args) = args else {
                            return Err(Error::runtime("construction splat expects an array"));
                        };
                        let class = pop(frame)?;
                        return self.construct(frame, class, args.as_ref().clone());
                    }
                    Instruction::Return => {
                        if !frame.handlers.is_empty() {
                            return Err(Error::runtime("handler leaked at Return"));
                        }
                        if frame.method_context.as_ref().is_some_and(|context| {
                            matches!(context.kind, MethodKind::Constructor)
                                && context.owner.superclass.is_some()
                        }) && !frame.receiver_initialized
                        {
                            return Err(Error::runtime(
                                "derived constructor must call super exactly once",
                            ));
                        }
                        let v = pop(frame)?;
                        return Ok(Step::Return(v));
                    }
                }
                Ok(Step::Continue)
            })();
            match step {
                Ok(Step::Continue) => {}
                Ok(Step::Return(value)) => {
                    let returning = frames.last_mut().expect("VM has a returning frame");
                    let value = returning.return_override.take().unwrap_or(value);
                    let initialize_caller_receiver = returning.initialize_caller_receiver_on_return;
                    if frames.len() == 1 {
                        return Ok(value);
                    }
                    self.call_depth = self.call_depth.saturating_sub(1);
                    let mut returning = frames.pop().expect("VM has a returning frame");
                    Self::recycle_frame_stack(std::mem::take(&mut returning.stack));
                    let parent = frames.last_mut().expect("returning frame has a caller");
                    if initialize_caller_receiver {
                        parent.receiver_initialized = true;
                    }
                    parent.stack.push(value);
                }
                Ok(Step::Call {
                    target,
                    args,
                    return_override,
                    initialize_caller_receiver,
                }) => {
                    let frame_count = frames.len();
                    let result = match target {
                        CallTarget::Value(callee) => call(self, &mut frames, callee, &args),
                        CallTarget::Receiver {
                            function,
                            receiver,
                            context,
                        } => call_with_receiver(
                            self,
                            &mut frames,
                            function,
                            &receiver,
                            &args,
                            Some(context),
                        ),
                    };
                    Self::recycle_call_arguments(args);
                    match result {
                        Ok(()) => {
                            if let Some(value) = return_override {
                                if frames.len() > frame_count {
                                    frames
                                        .last_mut()
                                        .expect("constructor call has a frame")
                                        .return_override = Some(value);
                                } else {
                                    let caller =
                                        frames.last_mut().expect("constructor call has a caller");
                                    caller.stack.pop();
                                    caller.stack.push(value);
                                }
                            }
                            if initialize_caller_receiver {
                                if frames.len() > frame_count {
                                    frames
                                        .last_mut()
                                        .expect("super call has a frame")
                                        .initialize_caller_receiver_on_return = true;
                                } else {
                                    frames
                                        .last_mut()
                                        .expect("super call has a caller")
                                        .receiver_initialized = true;
                                }
                            }
                        }
                        Err(error) => {
                            let include_current_call = !error.labels().is_empty();
                            let span = frames.last().and_then(|frame| {
                                Self::source_span(frame, frame.pc.saturating_sub(1))
                            });
                            let error = error.with_span_if_missing(span);
                            let error = Self::with_call_stack(error, &frames, include_current_call);
                            if !handle_error(self, &mut frames, &error) {
                                return Err(error);
                            }
                        }
                    }
                }
                Err(error) => {
                    let span = frames
                        .last()
                        .and_then(|frame| Self::source_span(frame, frame.pc.saturating_sub(1)));
                    let error = error.with_span_if_missing(span);
                    let error = Self::with_call_stack(error, &frames, false);
                    if !handle_error(self, &mut frames, &error) {
                        return Err(error);
                    }
                }
            }
        }
    }
}
fn value_function(value: Value, name: &str) -> Result<Rc<Function>, Error> {
    let Value::Function(function) = value else {
        return Err(Error::runtime(format!(
            "class member '{name}' is not a function"
        )));
    };
    Ok(function)
}

fn bound_method(function: Rc<Function>, receiver: Value, context: MethodContext) -> Value {
    Value::Function(Rc::new(Function {
        inner: FunctionKind::BoundMethod {
            function,
            receiver,
            context,
        },
    }))
}

fn receiver_bound(function: &Function) -> bool {
    matches!(function.inner, FunctionKind::ReceiverBound { .. })
}

fn find_constructor(class: &Rc<Class>) -> Option<(Rc<Class>, Rc<Function>)> {
    let mut current = Some(class.clone());
    while let Some(class) = current {
        if let Some(constructor) = &class.constructor {
            return Some((class.clone(), constructor.clone()));
        }
        current = class.superclass.clone();
    }
    None
}

fn function_uses_super(function: &Function) -> bool {
    match &function.inner {
        FunctionKind::Bytecode { chunk, .. } => chunk.code.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::SuperCall(_) | Instruction::SuperCallSpread
            )
        }),
        FunctionKind::ReceiverBound { function, .. } => function_uses_super(function),
        _ => false,
    }
}

fn find_instance_method(class: &Rc<Class>, name: &str) -> Option<(Rc<Class>, Rc<Function>)> {
    let mut current = Some(class.clone());
    while let Some(class) = current {
        if let Some(function) = class.instance_methods.get(name) {
            return Some((class.clone(), function.clone()));
        }
        current = class.superclass.clone();
    }
    None
}

fn find_static_method(class: &Rc<Class>, name: &str) -> Option<(Rc<Class>, Rc<Function>)> {
    let mut current = Some(class.clone());
    while let Some(class) = current {
        if let Some(function) = class.static_methods.get(name) {
            return Some((class.clone(), function.clone()));
        }
        current = class.superclass.clone();
    }
    None
}

fn find_static_field(class: &Rc<Class>, name: &str) -> Option<Value> {
    let mut current = Some(class.clone());
    while let Some(class) = current {
        if let Some(value) = class.static_fields.borrow().get(name).cloned() {
            return Some(value);
        }
        current = class.superclass.clone();
    }
    None
}

fn unbound_method(owner: &str, name: &str) -> Value {
    Value::Function(Rc::new(Function {
        inner: FunctionKind::UnboundMethod {
            owner: Rc::from(owner),
            name: Rc::from(name),
        },
    }))
}

fn member_call_target(target: Value, name: &str) -> Result<(CallTarget, bool), Error> {
    match target {
        Value::Instance(instance) => {
            if let Some(value) = instance.fields.borrow().get(name).cloned() {
                return Ok((CallTarget::Value(value), true));
            }
            let (owner, function) =
                find_instance_method(&instance.class, name).ok_or_else(|| {
                    Error::runtime(format!(
                        "instance of {} has no member '{name}'",
                        instance.class.name
                    ))
                })?;
            Ok((
                CallTarget::Receiver {
                    function,
                    receiver: Value::Instance(instance),
                    context: MethodContext {
                        owner,
                        name: Rc::from(name),
                        kind: MethodKind::Instance,
                    },
                },
                false,
            ))
        }
        Value::Class(class) => {
            if let Some(value) = find_static_field(&class, name) {
                return Ok((CallTarget::Value(value), true));
            }
            let (owner, function) = find_static_method(&class, name).ok_or_else(|| {
                Error::runtime(format!(
                    "class {} has no static member '{name}'",
                    class.name
                ))
            })?;
            Ok((
                CallTarget::Receiver {
                    function,
                    receiver: Value::Class(class),
                    context: MethodContext {
                        owner,
                        name: Rc::from(name),
                        kind: MethodKind::Static,
                    },
                },
                false,
            ))
        }
        value => member_value(value, name, true).map(|value| (CallTarget::Value(value), true)),
    }
}

fn member_value(target: Value, name: &str, bind_method: bool) -> Result<Value, Error> {
    match target {
        Value::Map(map) => map
            .get(name)
            .cloned()
            .ok_or_else(|| Error::runtime(format!("map key '{name}' not found"))),
        Value::Error(error) => match name {
            "code" => Ok(Value::String(error.code.clone())),
            "message" => Ok(Value::String(error.message.clone())),
            "data" => Ok(error.data.clone()),
            "cause" => Ok(error
                .cause
                .as_ref()
                .map_or(Value::Nil, |cause| Value::Error(cause.clone()))),
            _ => Err(Error::runtime(format!("error field '{name}' not found"))),
        },
        Value::Instance(instance) => {
            if let Some(value) = instance.fields.borrow().get(name).cloned() {
                return Ok(value);
            }
            let (owner, function) =
                find_instance_method(&instance.class, name).ok_or_else(|| {
                    Error::runtime(format!(
                        "instance of {} has no member '{name}'",
                        instance.class.name
                    ))
                })?;
            if bind_method || receiver_bound(&function) {
                Ok(bound_method(
                    function,
                    Value::Instance(instance),
                    MethodContext {
                        owner,
                        name: Rc::from(name),
                        kind: MethodKind::Instance,
                    },
                ))
            } else {
                Ok(unbound_method(&instance.class.name, name))
            }
        }
        Value::Class(class) => {
            if let Some(value) = find_static_field(&class, name) {
                return Ok(value);
            }
            let (owner, function) = find_static_method(&class, name).ok_or_else(|| {
                Error::runtime(format!(
                    "class {} has no static member '{name}'",
                    class.name
                ))
            })?;
            if bind_method || receiver_bound(&function) {
                Ok(bound_method(
                    function,
                    Value::Class(class),
                    MethodContext {
                        owner,
                        name: Rc::from(name),
                        kind: MethodKind::Static,
                    },
                ))
            } else {
                Ok(unbound_method(&class.name, name))
            }
        }
        _ => Err(Error::runtime(
            "member access expects a map, error, class, or instance",
        )),
    }
}

fn set_receiver_member(
    target: Value,
    name: &str,
    value: Value,
    limits: ResourceLimits,
) -> Result<bool, Error> {
    check_value_resources(&value, limits)?;
    match target {
        Value::Instance(instance) => {
            let mut fields = instance.fields.borrow_mut();
            let inserted = !fields.contains_key(name);
            if inserted {
                let next_len = fields.len().checked_add(1).ok_or_else(|| {
                    Error::resource(ResourceLimit::MapEntries, "map is too large")
                })?;
                check_map_resource(next_len, limits)?;
            }
            fields.insert(name.to_owned(), value);
            Ok(inserted)
        }
        Value::Class(class) => {
            let mut fields = class.static_fields.borrow_mut();
            let inserted = !fields.contains_key(name);
            if inserted {
                let next_len = fields.len().checked_add(1).ok_or_else(|| {
                    Error::resource(ResourceLimit::MapEntries, "map is too large")
                })?;
                check_map_resource(next_len, limits)?;
            }
            fields.insert(name.to_owned(), value);
            Ok(inserted)
        }
        _ => Err(Error::runtime(
            "receiver field writes require a class or instance receiver",
        )),
    }
}

fn call(vm: &mut Vm, frames: &mut Vec<Frame>, callee: Value, args: &[Value]) -> Result<(), Error> {
    call_with_context(vm, frames, callee, args, None)
}

// Most class calls have at most two explicit arguments. Keep receiver
// prepending off the allocator path for those calls while leaving uncommon
// larger arities on a straightforward fallback.
fn call_with_receiver(
    vm: &mut Vm,
    frames: &mut Vec<Frame>,
    function: Rc<Function>,
    receiver: &Value,
    args: &[Value],
    method_context: Option<MethodContext>,
) -> Result<(), Error> {
    match args {
        [] => call_with_context(
            vm,
            frames,
            Value::Function(function),
            std::slice::from_ref(receiver),
            method_context,
        ),
        [first] => call_with_context(
            vm,
            frames,
            Value::Function(function),
            &[receiver.clone(), first.clone()],
            method_context,
        ),
        [first, second] => call_with_context(
            vm,
            frames,
            Value::Function(function),
            &[receiver.clone(), first.clone(), second.clone()],
            method_context,
        ),
        _ => {
            let mut bound_args = Vec::with_capacity(args.len() + 1);
            bound_args.push(receiver.clone());
            bound_args.extend_from_slice(args);
            call_with_context(
                vm,
                frames,
                Value::Function(function),
                &bound_args,
                method_context,
            )
        }
    }
}

fn call_with_context(
    vm: &mut Vm,
    frames: &mut Vec<Frame>,
    callee: Value,
    args: &[Value],
    method_context: Option<MethodContext>,
) -> Result<(), Error> {
    match callee {
        Value::Function(function) => match &function.inner {
            FunctionKind::Native {
                function,
                allocation_profile,
            } => {
                let value = function(args)?;
                if vm.value_limits_active && value_needs_resource_check(&value) {
                    check_value_resources(&value, vm.resource_limits)?;
                }
                if let Some(allocation_profile) = allocation_profile {
                    vm.record_managed_allocation(allocation_profile(args, &value));
                }
                frames
                    .last_mut()
                    .expect("call has a caller frame")
                    .stack
                    .push(value);
            }
            FunctionKind::ResourceBuiltin {
                function,
                allocation_profile,
            } => {
                let value = function(args, vm.resource_limits)?;
                if vm.value_limits_active && value_needs_resource_check(&value) {
                    check_value_resources(&value, vm.resource_limits)?;
                }
                if let Some(allocation_profile) = allocation_profile {
                    vm.record_managed_allocation(allocation_profile(args, &value));
                }
                frames
                    .last_mut()
                    .expect("call has a caller frame")
                    .stack
                    .push(value);
            }
            FunctionKind::BoundMethod {
                function: method,
                receiver,
                context,
            } => call_with_receiver(
                vm,
                frames,
                method.clone(),
                receiver,
                args,
                Some(context.clone()),
            )?,
            FunctionKind::UnboundMethod { owner, name } => {
                return Err(Error::runtime(format!(
                    "method {owner}.{name} requires a receiver; call it through member syntax"
                )));
            }
            FunctionKind::Bytecode {
                params,
                required,
                rest,
                receiver,
                chunk,
                debug_info,
                execution_plan,
                fast_parameters,
                env: captured,
                ..
            } => {
                if vm.call_depth >= vm.max_call_depth {
                    return Err(Error::resource(
                        ResourceLimit::CallDepth,
                        format!(
                            "maximum QuickCoffee call depth of {} exceeded",
                            vm.max_call_depth
                        ),
                    ));
                }
                if args.len() < *required || (rest.is_none() && args.len() > params.len()) {
                    let hidden = usize::from(*receiver);
                    return Err(Error::runtime(format!(
                        "expected {}{} arguments, got {}",
                        required.saturating_sub(hidden),
                        if rest.is_some() { " or more" } else { "" },
                        args.len().saturating_sub(hidden)
                    )));
                }
                let binding_slots = execution_plan.as_ref().and_then(|plan| plan.slots(chunk));
                let fast_locals = fast_parameters.as_ref().zip(binding_slots.as_ref()).map(
                    |(parameter_slots, binding_slots)| {
                        let mut values = vec![None; binding_slots.local_names.len()];
                        for (slot, value) in parameter_slots.iter().zip(args) {
                            if let Some(slot) = slot {
                                values[*slot] = Some(value.clone());
                            }
                        }
                        values
                    },
                );
                let environment_elidable = fast_locals
                    .as_ref()
                    .is_some_and(|locals| locals.iter().all(Option::is_some));
                let shared_environment = fast_locals.is_none()
                    && binding_slots
                        .as_ref()
                        .is_some_and(|slots| slots.shared_environment);
                let local = if environment_elidable {
                    captured.clone()
                } else if shared_environment {
                    env_with_unset_slots(
                        captured.clone(),
                        &binding_slots
                            .as_ref()
                            .expect("shared environments require binding slots")
                            .local_names,
                    )
                } else {
                    env(Some(captured.clone()))
                };
                // ExecutionStats models one logical lexical frame per bytecode
                // call even when an isolated fast frame needs no physical Env.
                vm.record_environment_allocation();
                if fast_locals.is_none() {
                    for (index, pattern) in params.iter().enumerate() {
                        let value = args.get(index).cloned().unwrap_or(Value::Nil);
                        let mut bindings = vec![];
                        let snapshot = local.borrow().snapshot();
                        if let Err(error) = bind_pattern(
                            vm,
                            pattern,
                            Some(&value),
                            &mut bindings,
                            &local,
                            debug_info.as_ref(),
                            execution_plan.as_ref(),
                        ) {
                            local.borrow_mut().restore(snapshot);
                            return Err(error);
                        }
                        let mut environment = local.borrow_mut();
                        for (key, value) in bindings {
                            environment.set_local(&key, value);
                        }
                    }
                    if let Some(rest) = rest {
                        let value = Value::Array(Rc::new(args[params.len()..].to_vec()));
                        vm.record_shallow_value_allocation(1, &value);
                        local.borrow_mut().set_local(rest, value);
                    }
                }
                let bindings = match (binding_slots, fast_locals) {
                    (Some(slots), Some(locals)) => FrameBindings::Fast { slots, locals },
                    (Some(slots), None) if slots.shared_environment => FrameBindings::Shared(slots),
                    (Some(slots), None) => FrameBindings::Guarded(slots),
                    (None, None) => FrameBindings::Raw,
                    (None, Some(_)) => unreachable!("fast locals require a binding plan"),
                };
                let receiver_initialized = !method_context.as_ref().is_some_and(|context| {
                    matches!(context.kind, MethodKind::Constructor)
                        && context.owner.superclass.is_some()
                });
                frames.push(Frame {
                    chunk: chunk.clone(),
                    pc: 0,
                    stack: Vm::take_frame_stack(),
                    iterators: vec![],
                    handlers: vec![],
                    debug_info: debug_info.clone(),
                    execution_plan: execution_plan.clone(),
                    env: local,
                    bindings,
                    receiver: receiver.then(|| args[0].clone()),
                    method_context,
                    receiver_initialized,
                    super_called: false,
                    initialize_caller_receiver_on_return: false,
                    return_override: None,
                });
                vm.call_depth += 1;
                vm.call_depth_peak = vm.call_depth_peak.max(vm.call_depth);
            }
            FunctionKind::ReceiverBound {
                function,
                captured_receiver,
            } => {
                if let Some(receiver) = captured_receiver {
                    call_with_receiver(
                        vm,
                        frames,
                        function.clone(),
                        receiver,
                        args,
                        method_context,
                    )?;
                } else {
                    call_with_context(
                        vm,
                        frames,
                        Value::Function(function.clone()),
                        args,
                        method_context,
                    )?;
                }
            }
            FunctionKind::ContextualNative { function } => {
                let mut context = NativeCallContext {
                    cancellation: vm.cancellation.clone(),
                    resource_limits: vm.resource_limits,
                    host_bindings: vm.host_bindings.clone(),
                    fuel_remaining: vm.fuel,
                    managed_objects_allocated: 0,
                    managed_bytes_allocated: 0,
                };
                let result = function(&mut context, args);
                vm.fuel = context.fuel_remaining;
                vm.record_managed_allocation(ManagedAllocation {
                    legacy_value_allocations: 0,
                    objects: context.managed_objects_allocated,
                    bytes: context.managed_bytes_allocated,
                });
                let value = result?;
                if vm.value_limits_active && value_needs_resource_check(&value) {
                    check_value_resources(&value, vm.resource_limits)?;
                }
                frames
                    .last_mut()
                    .expect("call has a caller frame")
                    .stack
                    .push(value);
            }
        },
        _ => return Err(Error::runtime("attempted to call a non-function")),
    }
    Ok(())
}
fn handle_error(vm: &mut Vm, frames: &mut Vec<Frame>, error: &Error) -> bool {
    if error.kind() == ErrorKind::Resource {
        return false;
    }
    loop {
        let Some(frame) = frames.last_mut() else {
            return false;
        };
        if let Some(handler) = frame.handlers.pop() {
            frame.stack.truncate(handler.stack_depth);
            frame.iterators.truncate(handler.iterator_depth);
            let caught = error.catch_value();
            vm.record_shallow_value_allocation(1, &caught);
            frame.env.borrow_mut().set_local(&handler.name, caught);
            frame.pc = handler.catch_pc;
            return true;
        }
        let has_caller = frames.len() > 1;
        if has_caller {
            vm.call_depth = vm.call_depth.saturating_sub(1);
        }
        let mut discarded = frames.pop().expect("VM has a failing frame");
        if has_caller {
            Vm::recycle_frame_stack(std::mem::take(&mut discarded.stack));
        }
    }
}
fn pop(f: &mut Frame) -> Result<Value, Error> {
    f.stack
        .pop()
        .ok_or_else(|| Error::runtime("stack underflow"))
}
fn take(f: &mut Frame, n: usize) -> Result<Vec<Value>, Error> {
    if f.stack.len() < n {
        return Err(Error::runtime("stack underflow"));
    }
    Ok(f.stack.split_off(f.stack.len() - n))
}
fn number(v: Value) -> Result<f64, Error> {
    v.as_number()
        .ok_or_else(|| Error::runtime("expected number"))
}
fn decimal_to_exact_number(value: &Decimal, limits: ResourceLimits) -> Result<f64, Error> {
    check_decimal_power_growth(&BigInt::from(1_u8), value.scale, limits)?;
    let mut numerator = value.inner().clone();
    let mut denominator = decimal_power_of_ten(value.scale);
    let gcd = numerator.gcd(&denominator);
    numerator /= &gcd;
    denominator /= gcd;
    while denominator.is_even() {
        denominator /= 2;
    }
    if denominator != BigInt::from(1_u8) || numerator.bits() > 53 {
        return Err(Error::runtime(
            "decimal is not exactly representable as a Number",
        ));
    }
    let number = value
        .to_plain_string()
        .parse::<f64>()
        .map_err(|_| Error::runtime("decimal is not representable as a Number"))?;
    if !number.is_finite() || (!value.inner().is_zero() && number == 0.) {
        Err(Error::runtime("decimal is outside the finite Number range"))
    } else {
        Ok(number)
    }
}
#[derive(Clone, Copy)]
enum Aggregate {
    Sum,
    Min,
    Max,
}
fn aggregate_numeric(
    xs: &[Value],
    name: &str,
    aggregate: Aggregate,
    limits: ResourceLimits,
) -> Result<Value, Error> {
    if xs.len() != 1 {
        return Err(Error::runtime(format!("{name} expects one array")));
    }
    let Value::Array(values) = &xs[0] else {
        return Err(Error::runtime(format!("{name} expects an array")));
    };
    if values.is_empty() {
        return if matches!(aggregate, Aggregate::Sum) {
            Ok(Value::Number(0.))
        } else {
            Err(Error::runtime(format!("{name} expects a non-empty array")))
        };
    }
    match &values[0] {
        Value::Number(_) => {
            let numbers = values
                .iter()
                .map(|value| match value {
                    Value::Number(value) if value.is_finite() => Ok(*value),
                    Value::Integer(_) => Err(Error::runtime(format!(
                        "{name} cannot mix number and integer elements"
                    ))),
                    _ => Err(Error::runtime(format!(
                        "{name} expects finite numeric elements"
                    ))),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let value = match aggregate {
                Aggregate::Sum => numbers.into_iter().sum(),
                Aggregate::Min => numbers.into_iter().reduce(f64::min).expect("non-empty"),
                Aggregate::Max => numbers.into_iter().reduce(f64::max).expect("non-empty"),
            };
            Ok(Value::Number(value))
        }
        Value::Integer(_) => {
            let integers = values
                .iter()
                .map(|value| match value {
                    Value::Integer(value) => Ok(value.inner()),
                    Value::Number(_) => Err(Error::runtime(format!(
                        "{name} cannot mix number and integer elements"
                    ))),
                    _ => Err(Error::runtime(format!("{name} expects numeric elements"))),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let value = match aggregate {
                Aggregate::Sum => integers.into_iter().try_fold(
                    BigInt::zero(),
                    |sum, value| -> Result<BigInt, Error> {
                        let sum = sum + value;
                        check_integer_resource(&sum, limits)?;
                        Ok(sum)
                    },
                )?,
                Aggregate::Min => (*integers.into_iter().min().expect("non-empty")).clone(),
                Aggregate::Max => (*integers.into_iter().max().expect("non-empty")).clone(),
            };
            Ok(Value::Integer(Rc::new(resource_integer(value, limits)?)))
        }
        Value::Decimal(_) => {
            let decimals = values
                .iter()
                .map(|value| match value {
                    Value::Decimal(value) => Ok(value.as_ref()),
                    Value::Number(_) | Value::Integer(_) => Err(Error::runtime(format!(
                        "{name} cannot mix number, integer, and decimal elements"
                    ))),
                    _ => Err(Error::runtime(format!("{name} expects numeric elements"))),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let value = match aggregate {
                Aggregate::Sum => decimals
                    .into_iter()
                    .try_fold(Decimal::from(0_i64), |sum, value| {
                        decimal_add(&sum, value, limits)
                    })?,
                Aggregate::Min | Aggregate::Max => {
                    let mut decimals = decimals.into_iter();
                    let mut selected = decimals.next().expect("non-empty");
                    for candidate in decimals {
                        let ordering = decimal_cmp_resource(candidate, selected, limits)?;
                        if (matches!(aggregate, Aggregate::Min) && ordering.is_lt())
                            || (matches!(aggregate, Aggregate::Max) && ordering.is_gt())
                        {
                            selected = candidate;
                        }
                    }
                    selected.clone()
                }
            };
            Ok(Value::Decimal(Rc::new(value)))
        }
        _ => Err(Error::runtime(format!("{name} expects numeric elements"))),
    }
}
fn string_value(value: &Value) -> String {
    match value {
        Value::Integer(value) => value.to_decimal_string(),
        Value::Decimal(value) => value.to_plain_string(),
        value => value.to_string(),
    }
}
fn numeric_range(
    start: f64,
    end: f64,
    inclusive: bool,
    limits: ResourceLimits,
) -> Result<Value, Error> {
    if !start.is_finite()
        || !end.is_finite()
        || start.fract() != 0.
        || end.fract() != 0.
        || start < i64::MIN as f64
        || start > i64::MAX as f64
        || end < i64::MIN as f64
        || end > i64::MAX as f64
    {
        return Err(Error::runtime("range bounds must be finite integers"));
    }
    let start = start as i64;
    let end = end as i64;
    let start_i = i128::from(start);
    let end_i = i128::from(end);
    let (count, direction) = if start <= end {
        let limit = if inclusive { end_i + 1 } else { end_i };
        (limit - start_i, 1_i128)
    } else {
        let limit = if inclusive { end_i - 1 } else { end_i };
        (start_i - limit, -1_i128)
    };
    if count > MAX_RANGE_ITEMS {
        return Err(Error::runtime("range is too large"));
    }
    check_array_resource(count as usize, limits)?;
    Ok(Value::Array(Rc::new(
        (0..count as usize)
            .map(|offset| Value::Number((start_i + offset as i128 * direction) as f64))
            .collect(),
    )))
}
fn range_values(
    start: Value,
    end: Value,
    inclusive: bool,
    limits: ResourceLimits,
) -> Result<Value, Error> {
    match (start, end) {
        (Value::Number(start), Value::Number(end)) => numeric_range(start, end, inclusive, limits),
        (Value::Integer(start), Value::Integer(end)) => {
            let ascending = start <= end;
            let mut count = if ascending {
                end.inner() - start.inner()
            } else {
                start.inner() - end.inner()
            };
            if inclusive {
                count += 1;
            }
            let count = count
                .to_usize()
                .filter(|count| *count <= MAX_RANGE_ITEMS as usize)
                .ok_or_else(|| Error::runtime("range is too large"))?;
            check_array_resource(count, limits)?;
            let mut current = start.inner().clone();
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                values.push(Value::Integer(Rc::new(Integer(current.clone()))));
                if ascending {
                    current += 1;
                } else {
                    current -= 1;
                }
            }
            Ok(Value::Array(Rc::new(values)))
        }
        (Value::Number(_) | Value::Integer(_), Value::Number(_) | Value::Integer(_)) => Err(
            Error::runtime("range bounds cannot mix number and integer values"),
        ),
        _ => Err(Error::runtime(
            "range bounds must have the same numeric type",
        )),
    }
}
fn array_iteration_step(value: Value) -> Result<i64, Error> {
    let step = match value {
        Value::Number(step)
            if step.is_finite()
                && step.fract() == 0.
                && step >= i64::MIN as f64
                && step <= i64::MAX as f64 =>
        {
            step as i64
        }
        Value::Integer(step) => step
            .as_i64()
            .ok_or_else(|| Error::runtime("for by step must fit in a signed 64-bit integer"))?,
        _ => {
            return Err(Error::runtime(
                "for by step must be a non-zero finite integer",
            ));
        }
    };
    if step == 0 {
        Err(Error::runtime(
            "for by step must be a non-zero finite integer",
        ))
    } else {
        Ok(step)
    }
}
fn advance_position(position: &mut usize, step: i64) {
    if step >= 0 {
        *position = position.saturating_add(step as usize);
    } else {
        let amount = step.unsigned_abs() as usize;
        *position = position.checked_sub(amount).unwrap_or(usize::MAX);
    }
}
fn truth(v: Value) -> Result<bool, Error> {
    v.as_bool()
        .ok_or_else(|| Error::runtime("condition must be bool"))
}
fn push_integer(f: &mut Frame, value: BigInt, limits: ResourceLimits) -> Result<(), Error> {
    f.stack
        .push(Value::Integer(Rc::new(resource_integer(value, limits)?)));
    Ok(())
}
// Keep the full host policy borrowed across generic Number operations. Copying
// ResourceLimits here penalizes the scalar hot path even when no exact value is involved.
fn numeric_update(f: &mut Frame, increment: bool, limits: &ResourceLimits) -> Result<bool, Error> {
    let managed = match pop(f)? {
        Value::Number(value) => {
            f.stack.push(Value::Number(if increment {
                value + 1.
            } else {
                value - 1.
            }));
            false
        }
        Value::Integer(value) => {
            push_integer(
                f,
                if increment {
                    value.inner() + 1
                } else {
                    value.inner() - 1
                },
                *limits,
            )?;
            true
        }
        Value::Decimal(value) => {
            f.stack.push(Value::Decimal(Rc::new(decimal_add(
                &value,
                &Decimal::from(if increment { 1 } else { -1 }),
                *limits,
            )?)));
            true
        }
        _ => {
            return Err(Error::runtime(
                "update operand must be number, integer, or decimal",
            ));
        }
    };
    Ok(managed)
}
fn numeric_binary(
    f: &mut Frame,
    limits: &ResourceLimits,
    number_op: impl FnOnce(f64, f64) -> f64,
    integer_op: impl FnOnce(&BigInt, &BigInt, ResourceLimits) -> Result<BigInt, Error>,
    decimal_op: impl FnOnce(&Decimal, &Decimal, ResourceLimits) -> Result<Decimal, Error>,
) -> Result<bool, Error> {
    let b = pop(f)?;
    let a = pop(f)?;
    let managed = match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            f.stack.push(Value::Number(number_op(a, b)));
            false
        }
        (Value::Integer(a), Value::Integer(b)) => {
            push_integer(f, integer_op(a.inner(), b.inner(), *limits)?, *limits)?;
            true
        }
        (Value::Decimal(a), Value::Decimal(b)) => {
            f.stack
                .push(Value::Decimal(Rc::new(decimal_op(&a, &b, *limits)?)));
            true
        }
        (
            Value::Number(_) | Value::Integer(_) | Value::Decimal(_),
            Value::Number(_) | Value::Integer(_) | Value::Decimal(_),
        ) => {
            return Err(Error::runtime(
                "cannot mix number, integer, and decimal operands",
            ));
        }
        _ => {
            return Err(Error::runtime(
                "expected matching number or integer operands",
            ));
        }
    };
    Ok(managed)
}
fn integer_div(a: &BigInt, b: &BigInt) -> Result<BigInt, Error> {
    if b.is_zero() {
        Err(Error::runtime("integer division by zero"))
    } else {
        Ok(a / b)
    }
}
fn integer_rem(a: &BigInt, b: &BigInt) -> Result<BigInt, Error> {
    if b.is_zero() {
        Err(Error::runtime("integer remainder by zero"))
    } else {
        Ok(a % b)
    }
}
fn integer_floor_div(a: &BigInt, b: &BigInt) -> Result<BigInt, Error> {
    let quotient = integer_div(a, b)?;
    let remainder = a % b;
    if !remainder.is_zero() && a.is_negative() != b.is_negative() {
        Ok(quotient - 1)
    } else {
        Ok(quotient)
    }
}
fn integer_modulo(a: &BigInt, b: &BigInt) -> Result<BigInt, Error> {
    let remainder = integer_rem(a, b)?;
    if !remainder.is_zero() && remainder.is_negative() != b.is_negative() {
        Ok(remainder + b)
    } else {
        Ok(remainder)
    }
}
fn integer_pow(a: &BigInt, b: &BigInt, limits: ResourceLimits) -> Result<BigInt, Error> {
    let exponent = b
        .to_u32()
        .ok_or_else(|| Error::runtime("integer exponent must be a non-negative 32-bit integer"))?;
    if a.bits().saturating_mul(u64::from(exponent)) > integer_bit_limit(limits) {
        return Err(Error::resource(
            ResourceLimit::IntegerBits,
            format!(
                "integer magnitude exceeds {} bits",
                integer_bit_limit(limits)
            ),
        ));
    }
    Ok(a.pow(exponent))
}

fn integer_mul_resource(a: &BigInt, b: &BigInt, limits: ResourceLimits) -> Result<BigInt, Error> {
    let minimum_bits = a.bits().saturating_add(b.bits()).saturating_sub(1);
    if !a.is_zero() && !b.is_zero() && minimum_bits > integer_bit_limit(limits) {
        return Err(Error::resource(
            ResourceLimit::IntegerBits,
            format!(
                "integer magnitude exceeds {} bits",
                integer_bit_limit(limits)
            ),
        ));
    }
    Ok(a * b)
}
fn bit_integer(value: Value) -> Result<i32, Error> {
    let value = number(value)?;
    if !value.is_finite()
        || value.fract() != 0.
        || !(-2_147_483_648.0..=2_147_483_647.0).contains(&value)
    {
        return Err(Error::runtime(
            "bitwise operands must be finite 32-bit integers",
        ));
    }
    Ok(value as i32)
}
fn numeric_bit_binary(
    f: &mut Frame,
    limits: &ResourceLimits,
    number_op: impl FnOnce(i32, i32) -> i32,
    integer_op: impl FnOnce(&BigInt, &BigInt) -> BigInt,
) -> Result<(), Error> {
    let b = pop(f)?;
    let a = pop(f)?;
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let a = bit_integer(Value::Number(a))?;
            let b = bit_integer(Value::Number(b))?;
            f.stack.push(Value::Number(number_op(a, b) as f64));
        }
        (Value::Integer(a), Value::Integer(b)) => {
            push_integer(f, integer_op(a.inner(), b.inner()), *limits)?;
        }
        (Value::Number(_) | Value::Integer(_), Value::Number(_) | Value::Integer(_)) => {
            return Err(Error::runtime("cannot mix number and integer operands"));
        }
        _ => {
            return Err(Error::runtime(
                "bitwise operands must have the same numeric type",
            ));
        }
    }
    Ok(())
}
fn bit_shift(f: &mut Frame, op: impl Fn(i32, u32) -> i32) -> Result<(), Error> {
    let shift = bit_integer(pop(f)?)?;
    if !(0..32).contains(&shift) {
        return Err(Error::runtime(
            "shift count must be an integer from 0 to 31",
        ));
    }
    let value = bit_integer(pop(f)?)?;
    f.stack.push(Value::Number(op(value, shift as u32) as f64));
    Ok(())
}
fn numeric_shift(f: &mut Frame, right: bool, limits: &ResourceLimits) -> Result<(), Error> {
    let shift = pop(f)?;
    let value = pop(f)?;
    match (value, shift) {
        (Value::Number(value), Value::Number(shift)) => {
            let shift = bit_integer(Value::Number(shift))?;
            if !(0..32).contains(&shift) {
                return Err(Error::runtime(
                    "shift count must be an integer from 0 to 31",
                ));
            }
            let value = bit_integer(Value::Number(value))?;
            let result = if right {
                value.wrapping_shr(shift as u32)
            } else {
                value.wrapping_shl(shift as u32)
            };
            f.stack.push(Value::Number(result as f64));
        }
        (Value::Integer(value), Value::Integer(shift)) => {
            let shift = shift.inner().to_usize().ok_or_else(|| {
                Error::runtime("integer shift count must be a non-negative platform integer")
            })?;
            if shift as u64 > MAX_INTEGER_BITS {
                return Err(Error::resource(
                    ResourceLimit::IntegerBits,
                    format!(
                        "integer shift exceeds the {MAX_INTEGER_BITS}-bit implementation limit"
                    ),
                ));
            }
            if !right
                && value.inner().bits().saturating_add(shift as u64) > integer_bit_limit(*limits)
            {
                return Err(Error::resource(
                    ResourceLimit::IntegerBits,
                    format!(
                        "integer magnitude exceeds {} bits",
                        integer_bit_limit(*limits)
                    ),
                ));
            }
            push_integer(
                f,
                if right {
                    value.inner() >> shift
                } else {
                    value.inner() << shift
                },
                *limits,
            )?;
        }
        (Value::Number(_) | Value::Integer(_), Value::Number(_) | Value::Integer(_)) => {
            return Err(Error::runtime("cannot mix number and integer operands"));
        }
        _ => {
            return Err(Error::runtime(
                "shift operands must have the same numeric type",
            ));
        }
    }
    Ok(())
}
fn compare(f: &mut Frame, op: impl Fn(&Value, &Value) -> bool) -> Result<(), Error> {
    let b = pop(f)?;
    let a = pop(f)?;
    f.stack.push(Value::Bool(op(&a, &b)));
    Ok(())
}
fn numeric_order(
    f: &mut Frame,
    limits: &ResourceLimits,
    number_op: impl FnOnce(f64, f64) -> bool,
    integer_op: impl FnOnce(&BigInt, &BigInt) -> bool,
    decimal_op: impl FnOnce(std::cmp::Ordering) -> bool,
) -> Result<(), Error> {
    let b = pop(f)?;
    let a = pop(f)?;
    let result = match (a, b) {
        (Value::Number(a), Value::Number(b)) => number_op(a, b),
        (Value::Integer(a), Value::Integer(b)) => integer_op(a.inner(), b.inner()),
        (Value::Decimal(a), Value::Decimal(b)) => {
            decimal_op(decimal_cmp_resource(&a, &b, *limits)?)
        }
        (
            Value::Number(_) | Value::Integer(_) | Value::Decimal(_),
            Value::Number(_) | Value::Integer(_) | Value::Decimal(_),
        ) => {
            return Err(Error::runtime(
                "cannot order number, integer, and decimal values",
            ));
        }
        _ => {
            return Err(Error::runtime(
                "ordered comparison expects matching numeric types",
            ));
        }
    };
    f.stack.push(Value::Bool(result));
    Ok(())
}
fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Nil, Value::Nil) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::Integer(x), Value::Integer(y)) => x == y,
        (Value::Decimal(x), Value::Decimal(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| equal(a, b))
        }
        (Value::Map(x), Value::Map(y)) => {
            x.len() == y.len() && x.iter().all(|(k, a)| y.get(k).is_some_and(|b| equal(a, b)))
        }
        (Value::Error(x), Value::Error(y)) => {
            x.code == y.code
                && x.message == y.message
                && equal(&x.data, &y.data)
                && match (&x.cause, &y.cause) {
                    (None, None) => true,
                    (Some(x), Some(y)) => equal(&Value::Error(x.clone()), &Value::Error(y.clone())),
                    _ => false,
                }
        }
        (Value::Class(x), Value::Class(y)) => Rc::ptr_eq(x, y),
        (Value::Instance(x), Value::Instance(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}
fn jump(f: &mut Frame, delta: i32) -> Result<(), Error> {
    f.pc = (f.pc as i64 + delta as i64)
        .try_into()
        .map_err(|_| Error::runtime("invalid jump"))?;
    Ok(())
}

// Patterns without defaults can be checked without changing the environment.
// Once this predicate succeeds, collecting their bindings is infallible and
// callers can commit once without cloning the complete environment for rollback.
fn static_pattern_matches(pattern: &Pattern, value: &Value) -> bool {
    match pattern {
        Pattern::Ignore | Pattern::Bind(_) | Pattern::Rest(_) => true,
        Pattern::Default { .. } => false,
        Pattern::Array(patterns) => {
            let Value::Array(values) = value else {
                return false;
            };
            let rest_index = patterns
                .iter()
                .position(|pattern| matches!(pattern, Pattern::Rest(_)));
            let required_len = patterns
                .iter()
                .enumerate()
                .filter(|(_, pattern)| {
                    !matches!(pattern, Pattern::Default { .. } | Pattern::Rest(_))
                })
                .map(|(index, _)| index + 1)
                .max()
                .unwrap_or(0);
            let fixed_len = rest_index.unwrap_or(required_len);
            if values.len() < fixed_len || (rest_index.is_none() && values.len() > patterns.len()) {
                return false;
            }
            patterns.iter().enumerate().all(|(index, pattern)| {
                matches!(pattern, Pattern::Rest(_))
                    || values
                        .get(index)
                        .is_some_and(|value| static_pattern_matches(pattern, value))
            })
        }
        Pattern::Map(fields) => {
            let Value::Map(map) = value else {
                return false;
            };
            fields.iter().all(|(key, pattern)| {
                map.get(key)
                    .is_some_and(|value| static_pattern_matches(pattern, value))
            })
        }
        Pattern::MapRest { fields, .. } => {
            let Value::Map(map) = value else {
                return false;
            };
            fields.iter().all(|(key, pattern)| {
                map.get(key)
                    .is_some_and(|value| static_pattern_matches(pattern, value))
            })
        }
    }
}

fn collect_static_pattern_bindings(
    pattern: &Pattern,
    value: &Value,
) -> (Vec<(String, Value)>, ManagedAllocation) {
    let mut bindings = vec![];
    let mut allocations = ManagedAllocation::default();
    collect_static_pattern_bindings_into(pattern, value, &mut bindings, &mut allocations);
    (bindings, allocations)
}

fn collect_static_pattern_bindings_into(
    pattern: &Pattern,
    value: &Value,
    bindings: &mut Vec<(String, Value)>,
    allocations: &mut ManagedAllocation,
) {
    match pattern {
        Pattern::Ignore => {}
        Pattern::Bind(name) | Pattern::Rest(name) => {
            bindings.push((name.clone(), value.clone()));
        }
        Pattern::Default { .. } => {
            unreachable!("static pattern matching excludes defaults");
        }
        Pattern::Array(patterns) => {
            let Value::Array(values) = value else {
                unreachable!("static array pattern was prevalidated");
            };
            for (index, pattern) in patterns.iter().enumerate() {
                if let Pattern::Rest(name) = pattern {
                    let value = Value::Array(Rc::new(values[index..].to_vec()));
                    allocations.add(ManagedAllocation::legacy_shallow(1, &value));
                    bindings.push((name.clone(), value));
                    break;
                }
                collect_static_pattern_bindings_into(
                    pattern,
                    &values[index],
                    bindings,
                    allocations,
                );
            }
        }
        Pattern::Map(fields) => {
            let Value::Map(map) = value else {
                unreachable!("static map pattern was prevalidated");
            };
            for (key, pattern) in fields {
                collect_static_pattern_bindings_into(pattern, &map[key], bindings, allocations);
            }
        }
        Pattern::MapRest { fields, rest } => {
            let Value::Map(map) = value else {
                unreachable!("static map-rest pattern was prevalidated");
            };
            for (key, pattern) in fields {
                collect_static_pattern_bindings_into(pattern, &map[key], bindings, allocations);
            }
            let explicit_fields: BTreeSet<&str> =
                fields.iter().map(|(field, _)| field.as_str()).collect();
            let remaining = map
                .iter()
                .filter(|(key, _)| !explicit_fields.contains(key.as_str()))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            let value = Value::Map(Rc::new(remaining));
            allocations.add(ManagedAllocation::legacy_shallow(1, &value));
            bindings.push((rest.clone(), value));
        }
    }
}

fn bind_pattern(
    vm: &mut Vm,
    pattern: &Pattern,
    value: Option<&Value>,
    bindings: &mut Vec<(String, Value)>,
    env: &Env,
    debug_info: Option<&Rc<ProgramDebugInfo>>,
    execution_plan: Option<&Rc<ProgramExecutionPlan>>,
) -> Result<(), Error> {
    match pattern {
        Pattern::Ignore => value.map_or_else(
            || Err(Error::runtime("missing value for pattern")),
            |_| Ok(()),
        ),
        Pattern::Bind(name) => {
            let value = value.ok_or_else(|| Error::runtime("missing value for pattern"))?;
            bindings.push((name.clone(), value.clone()));
            if name != "_" {
                env.borrow_mut().set_local(name, value.clone());
            }
            Ok(())
        }
        Pattern::Rest(name) => {
            let value = value.ok_or_else(|| Error::runtime("missing value for rest pattern"))?;
            bindings.push((name.clone(), value.clone()));
            if name != "_" {
                env.borrow_mut().set_local(name, value.clone());
            }
            Ok(())
        }
        Pattern::Default { pattern, default } => {
            let value = match value {
                Some(value) if !matches!(value, Value::Nil) => value.clone(),
                _ => vm.eval_default(
                    default.clone(),
                    env.clone(),
                    debug_info.cloned(),
                    execution_plan.cloned(),
                )?,
            };
            bind_pattern(
                vm,
                pattern,
                Some(&value),
                bindings,
                env,
                debug_info,
                execution_plan,
            )
        }
        Pattern::Array(patterns) => {
            let Some(Value::Array(values)) = value else {
                return Err(Error::runtime("array destructuring expects an array"));
            };
            let rest_index = patterns
                .iter()
                .position(|pattern| matches!(pattern, Pattern::Rest(_)));
            let required_len = patterns
                .iter()
                .enumerate()
                .filter(|(_, pattern)| {
                    !matches!(pattern, Pattern::Default { .. } | Pattern::Rest(_))
                })
                .map(|(index, _)| index + 1)
                .max()
                .unwrap_or(0);
            let fixed_len = rest_index.unwrap_or(required_len);
            if values.len() < fixed_len || (rest_index.is_none() && values.len() > patterns.len()) {
                return Err(Error::runtime(format!(
                    "array destructuring expected {} values, got {}",
                    if rest_index.is_some() {
                        format!("at least {fixed_len}")
                    } else {
                        patterns.len().to_string()
                    },
                    values.len()
                )));
            }
            for (index, pattern) in patterns.iter().enumerate() {
                if let Pattern::Rest(name) = pattern {
                    let rest = Value::Array(Rc::new(values[index..].to_vec()));
                    vm.record_shallow_value_allocation(1, &rest);
                    bindings.push((name.clone(), rest.clone()));
                    if name != "_" {
                        env.borrow_mut().set_local(name, rest);
                    }
                    break;
                }
                bind_pattern(
                    vm,
                    pattern,
                    values.get(index),
                    bindings,
                    env,
                    debug_info,
                    execution_plan,
                )?;
            }
            Ok(())
        }
        Pattern::Map(fields) => {
            let Some(Value::Map(map)) = value else {
                return Err(Error::runtime("map destructuring expects a map"));
            };
            for (key, pattern) in fields {
                bind_pattern(
                    vm,
                    pattern,
                    map.get(key),
                    bindings,
                    env,
                    debug_info,
                    execution_plan,
                )
                .map_err(|error| {
                    if map.contains_key(key) {
                        error
                    } else {
                        Error::runtime(format!("map key '{key}' not found"))
                    }
                })?;
            }
            Ok(())
        }
        Pattern::MapRest { fields, rest } => {
            let Some(Value::Map(map)) = value else {
                return Err(Error::runtime("map destructuring expects a map"));
            };
            for (key, pattern) in fields.iter() {
                bind_pattern(
                    vm,
                    pattern,
                    map.get(key),
                    bindings,
                    env,
                    debug_info,
                    execution_plan,
                )
                .map_err(|error| {
                    if map.contains_key(key) {
                        error
                    } else {
                        Error::runtime(format!("map key '{key}' not found"))
                    }
                })?;
            }
            let explicit_fields: BTreeSet<&str> =
                fields.iter().map(|(field, _)| field.as_str()).collect();
            let mut remaining = BTreeMap::new();
            for (key, item) in map.iter() {
                if !explicit_fields.contains(key.as_str()) {
                    remaining.insert(key.clone(), item.clone());
                }
            }
            let rest_value = Value::Map(Rc::new(remaining));
            vm.record_shallow_value_allocation(1, &rest_value);
            bindings.push((rest.clone(), rest_value.clone()));
            if rest != "_" {
                env.borrow_mut().set_local(rest, rest_value);
            }
            Ok(())
        }
    }
}
fn index(vm: &mut Vm, target: Value, key: Value) -> Result<Value, Error> {
    match (target, key) {
        (Value::Array(xs), i @ (Value::Number(_) | Value::Integer(_))) => xs
            .get(sequence_index(
                integral_i128(&i, "array index")?,
                xs.len(),
                "array",
            )?)
            .cloned()
            .ok_or_else(|| Error::runtime("array index out of range")),
        (Value::String(text), i @ (Value::Number(_) | Value::Integer(_))) => {
            let character = text
                .chars()
                .nth(sequence_index(
                    integral_i128(&i, "string index")?,
                    text.chars().count(),
                    "string",
                )?)
                .ok_or_else(|| Error::runtime("string index out of range"))?;
            let value = Value::String(Rc::from(character.to_string()));
            vm.record_shallow_value_allocation(1, &value);
            Ok(value)
        }
        (Value::Map(m), Value::String(k)) => m
            .get(k.as_ref())
            .cloned()
            .ok_or_else(|| Error::runtime("map key not found")),
        _ => Err(Error::runtime("invalid index operation")),
    }
}
fn integral_i128(value: &Value, name: &str) -> Result<i128, Error> {
    match value {
        Value::Number(value)
            if value.is_finite()
                && value.fract() == 0.
                && *value >= i128::MIN as f64
                && *value <= i128::MAX as f64 =>
        {
            Ok(*value as i128)
        }
        Value::Integer(value) => value
            .inner()
            .to_i128()
            .ok_or_else(|| Error::runtime(format!("{name} is out of range"))),
        _ => Err(Error::runtime(format!("{name} must be an integer value"))),
    }
}
fn sequence_index(index: i128, len: usize, kind: &str) -> Result<usize, Error> {
    let len = len as i128;
    let resolved = if index < 0 { len + index } else { index };
    if resolved < 0 || resolved >= len {
        return Err(Error::runtime(format!("{kind} index out of range")));
    }
    Ok(resolved as usize)
}
fn slice(
    vm: &mut Vm,
    target: Value,
    start: Value,
    end: Value,
    inclusive: bool,
    limits: ResourceLimits,
) -> Result<Value, Error> {
    match target {
        Value::Array(values) => {
            let start = slice_bound(start, values.len(), "slice start")?;
            let mut end = slice_bound(end, values.len(), "slice end")?;
            if inclusive {
                end = end
                    .checked_add(1)
                    .ok_or_else(|| Error::runtime("inclusive slice end is too large"))?;
            }
            if start > end || end > values.len() {
                return Err(Error::runtime("slice bounds out of range"));
            }
            check_array_resource(end - start, limits)?;
            let value = Value::Array(Rc::new(values[start..end].to_vec()));
            vm.record_shallow_value_allocation(1, &value);
            Ok(value)
        }
        Value::String(text) => {
            let scalar_len = text.chars().count();
            let start = slice_bound(start, scalar_len, "slice start")?;
            let mut end = slice_bound(end, scalar_len, "slice end")?;
            if inclusive {
                end = end
                    .checked_add(1)
                    .ok_or_else(|| Error::runtime("inclusive slice end is too large"))?;
            }
            if start > end || end > scalar_len {
                return Err(Error::runtime("slice bounds out of range"));
            }
            let start_byte = text
                .char_indices()
                .nth(start)
                .map_or(text.len(), |(offset, _)| offset);
            let end_byte = text
                .char_indices()
                .nth(end)
                .map_or(text.len(), |(offset, _)| offset);
            check_string_len_resource(end_byte - start_byte, limits)?;
            let value = Value::String(Rc::from(&text[start_byte..end_byte]));
            vm.record_shallow_value_allocation(1, &value);
            Ok(value)
        }
        _ => Err(Error::runtime("slice expects an array or string")),
    }
}
fn slice_bound(value: Value, len: usize, name: &str) -> Result<usize, Error> {
    let value = integral_i128(&value, name)?;
    if value < -(len as i128) || value > len as i128 {
        return Err(Error::runtime(format!("{name} out of range")));
    }
    let index = if value < 0 {
        len as i128 + value
    } else {
        value
    };
    usize::try_from(index).map_err(|_| Error::runtime(format!("{name} out of range")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_bindings_view_preserves_legacy_vm_handle_width() {
        assert_eq!(
            std::mem::size_of::<Option<HostBindingsViewHandle>>(),
            std::mem::size_of::<Option<HostState>>()
        );
    }

    #[test]
    fn literal_replacement_length_rejects_arithmetic_overflow() {
        let limits = ResourceLimits::default().with_max_string_bytes(usize::MAX);
        let error = checked_replacement_output_len(usize::MAX, 1, 2, 1, limits).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Resource);
        assert_eq!(error.resource_limit(), Some(ResourceLimit::StringBytes));
    }

    #[test]
    fn static_pattern_fast_path_prevalidates_and_counts_rest_backings() {
        let pattern = Pattern::MapRest {
            fields: vec![
                ("id".to_owned(), Pattern::Bind("id".to_owned())),
                (
                    "items".to_owned(),
                    Pattern::Array(vec![
                        Pattern::Bind("head".to_owned()),
                        Pattern::Rest("tail".to_owned()),
                    ]),
                ),
            ],
            rest: "metadata".to_owned(),
        };
        let value = Value::map([
            ("extra", Value::from(5_f64)),
            ("id", Value::from(1_f64)),
            (
                "items",
                Value::array(vec![
                    Value::from(2_f64),
                    Value::from(3_f64),
                    Value::from(4_f64),
                ]),
            ),
        ]);

        assert!(static_pattern_matches(&pattern, &value));
        let (bindings, allocations) = collect_static_pattern_bindings(&pattern, &value);
        let bindings = bindings.into_iter().collect::<BTreeMap<_, _>>();
        assert_eq!(allocations.legacy_value_allocations, 2);
        assert_eq!(allocations.objects, 2);
        assert_eq!(allocations.bytes, 37);
        assert_eq!(bindings["id"].as_number(), Some(1_f64));
        assert_eq!(bindings["head"].as_number(), Some(2_f64));
        assert_eq!(bindings["tail"].as_array().unwrap().len(), 2);
        assert_eq!(
            bindings["metadata"].as_map().unwrap()["extra"].as_number(),
            Some(5_f64)
        );

        assert!(!static_pattern_matches(
            &pattern,
            &Value::map([("items", Value::array(vec![Value::from(2_f64)]))])
        ));
        assert!(!static_pattern_matches(
            &Pattern::Default {
                pattern: Box::new(Pattern::Bind("value".to_owned())),
                default: Rc::new(Chunk::default()),
            },
            &Value::Nil
        ));
    }

    fn assert_result_equal(left: Result<Value, Error>, right: Result<Value, Error>) {
        match (left, right) {
            (Ok(left), Ok(right)) => assert!(
                equal(&left, &right),
                "optimized value {left:?} differs from reference {right:?}"
            ),
            (Err(left), Err(right)) => {
                assert_eq!(left.kind(), right.kind());
                assert_eq!(left.message(), right.message());
                assert_eq!(left.labels(), right.labels());
                assert_eq!(left.resource_limit(), right.resource_limit());
            }
            (left, right) => panic!("optimized/reference result mismatch: {left:?} vs {right:?}"),
        }
    }

    fn assert_cached_and_reference(source: &str, fuel: u64) {
        let optimized = Engine::new()
            .compile_program_named("differential.coffee", source)
            .expect("differential source must compile");
        let reference = optimized.without_binding_slots();
        assert_eq!(optimized.disassemble(), reference.disassemble());
        assert_eq!(optimized.fingerprint(), reference.fingerprint());
        let mut optimized_context = Context::new().with_fuel(fuel);
        let mut reference_context = Context::new().with_fuel(fuel);
        let optimized_result = optimized_context.run_program(&optimized);
        let reference_result = reference_context.run_program(&reference);
        assert_result_equal(optimized_result, reference_result);
        assert_eq!(
            optimized_context.last_execution(),
            reference_context.last_execution()
        );
    }

    #[test]
    fn reusable_call_arguments_preserve_mixed_calls_and_failure_recovery() {
        fn reusable_capacity() -> usize {
            REUSABLE_CALL_ARGUMENTS.with(|reusable| reusable.borrow().capacity())
        }
        fn reusable_is_empty() -> bool {
            REUSABLE_CALL_ARGUMENTS.with(|reusable| reusable.borrow().is_empty())
        }

        REUSABLE_CALL_ARGUMENTS.with(|reusable| *reusable.borrow_mut() = Vec::new());
        let engine = Engine::new();
        let mixed_calls = engine
            .compile_program(
                "zero = -> 1\none = (value) -> value + 1\nmany = (head, tail...) -> head + sum(tail)\n[zero(), one(1), many(1, 2, 3), many([1, 2, 3]...), len([1, 2, 3])]",
            )
            .unwrap();
        let bad_call = engine
            .compile_program("one = (value) -> value + 1\none()")
            .unwrap();
        let large_native_call = engine
            .compile_program(
                "count_args(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17)",
            )
            .unwrap();
        let mut context = Context::new();
        context.add_native("count_args", |args| Ok(Value::from(args.len() as f64)));

        assert_eq!(
            context.run_program(&mixed_calls).unwrap().to_string(),
            "[1, 2, 6, 6, 3]"
        );
        let successful_stats = context.last_execution();
        assert!(reusable_is_empty());
        assert!(reusable_capacity() <= MAX_REUSABLE_CALL_ARGUMENTS);
        REUSABLE_CALL_ARGUMENTS.with(|reusable| {
            *reusable.borrow_mut() = Vec::with_capacity(4);
        });
        let initial_capacity = reusable_capacity();
        assert!(initial_capacity > 0);

        assert_eq!(
            context.run_program(&large_native_call).unwrap().to_string(),
            "17"
        );
        let post_large_capacity = reusable_capacity();
        assert!(post_large_capacity >= initial_capacity);
        assert!(post_large_capacity <= MAX_REUSABLE_CALL_ARGUMENTS);

        let error = context.run_program(&bad_call).unwrap_err();
        assert_eq!(error.message(), "expected 1 arguments, got 0");
        assert!(reusable_is_empty());
        assert!(reusable_capacity() <= MAX_REUSABLE_CALL_ARGUMENTS);

        assert_eq!(
            context.run_program(&mixed_calls).unwrap().to_string(),
            "[1, 2, 6, 6, 3]"
        );
        assert_eq!(context.last_execution(), successful_stats);
        assert_eq!(reusable_capacity(), post_large_capacity);
        REUSABLE_CALL_ARGUMENTS.with(|reusable| *reusable.borrow_mut() = Vec::new());
    }

    #[test]
    fn reusable_frame_stack_is_bounded_and_preserves_calls_errors_and_contexts() {
        fn reusable_capacity() -> usize {
            REUSABLE_FRAME_STACK.with(|reusable| reusable.borrow().capacity())
        }
        fn reusable_is_empty() -> bool {
            REUSABLE_FRAME_STACK.with(|reusable| reusable.borrow().is_empty())
        }

        REUSABLE_FRAME_STACK.with(|reusable| *reusable.borrow_mut() = Vec::new());
        let engine = Engine::new();
        let program = engine
            .compile_program(
                "step = (value) -> value + 1\nrecurse = (value) -> if value == 0 then step(value) else step(recurse(value - 1))\ntry recurse(3) catch problem then -1",
            )
            .unwrap();
        let failure = engine
            .compile_program("fail = -> missing\ntry fail() catch problem then problem.code")
            .unwrap();
        let retained_closure = engine
            .compile_program("make = ->\n  captured = 41\n  -> captured + 1\nkept = make()\nkept()")
            .unwrap();
        let nested_program = engine
            .compile_program("twice = (value) -> value * 2\ntwice(21)")
            .unwrap();
        let reentrant = engine
            .compile_program("wrap = -> host_nested()\nwrap()")
            .unwrap();

        let mut first = Context::new();
        assert_eq!(first.run_program(&program).unwrap().as_number(), Some(4.));
        let successful_stats = first.last_execution();
        assert!(reusable_is_empty());
        assert!(reusable_capacity() > 0);
        assert!(reusable_capacity() <= MAX_REUSABLE_FRAME_STACK);

        assert_eq!(
            first.run_program(&failure).unwrap().as_str(),
            Some("runtime")
        );
        assert!(reusable_is_empty());
        assert!(reusable_capacity() <= MAX_REUSABLE_FRAME_STACK);
        assert_eq!(first.run_program(&program).unwrap().as_number(), Some(4.));
        assert_eq!(first.last_execution(), successful_stats);
        assert_eq!(
            first.run_program(&retained_closure).unwrap().as_number(),
            Some(42.)
        );

        first.add_native("host_nested", move |_| {
            Context::new().run_program(&nested_program)
        });
        assert_eq!(
            first.run_program(&reentrant).unwrap().as_number(),
            Some(42.)
        );
        assert!(reusable_is_empty());
        assert!(reusable_capacity() <= MAX_REUSABLE_FRAME_STACK);

        let mut second = Context::new();
        assert_eq!(second.run_program(&program).unwrap().as_number(), Some(4.));
        assert_eq!(second.last_execution(), successful_stats);

        REUSABLE_FRAME_STACK.with(|reusable| *reusable.borrow_mut() = Vec::new());
        Vm::recycle_frame_stack(Vec::with_capacity(MAX_REUSABLE_FRAME_STACK + 1));
        assert!(reusable_is_empty());
        assert_eq!(reusable_capacity(), MAX_REUSABLE_FRAME_STACK);
        REUSABLE_FRAME_STACK.with(|reusable| *reusable.borrow_mut() = Vec::new());
    }

    #[test]
    fn binding_slots_match_reference_for_dynamic_shadowing_and_layouts() {
        assert_cached_and_reference(
            "x = 1\nf = ->\n  i = 0\n  out = []\n  while i < 2\n    out = [out..., x]\n    x = 2 if i == 0\n    i++\n  out\nf()",
            10_000,
        );
        assert_cached_and_reference(
            "f = (flag) ->\n  extra = 1 if flag\n  x = 2\n  extra ?= 0\n  x + extra\n[f(false), f(true), f(false)]",
            10_000,
        );
    }

    #[test]
    fn binding_slots_match_reference_for_patterns_closures_and_handlers() {
        assert_cached_and_reference(
            "base = 10\nmake = (offset = 1) ->\n  ([left, right], {factor}) ->\n    sum = 0\n    for value, index in [left, right]\n      sum += value * factor + index + base + offset\n    try\n      throw 'boom' if sum < 0\n      sum\n    catch error\n      0\nfn = make(2)\nfn([1, 2], {factor: 3})",
            20_000,
        );
        assert_cached_and_reference(
            "x = 1\ni = 0\nwhile i < 2\n  try\n    {x, missing} = {x: i + 2}\n  catch error\n    nil\n  i++\nx",
            10_000,
        );
        assert_cached_and_reference(
            "outer = 40\nf = ->\n  try\n    [outer, missing] = [1]\n  catch error\n    outer\nf()",
            10_000,
        );
        assert_cached_and_reference(
            "side = 40\nf = ->\n  try\n    [local = (side = 1), {needed}] = [nil, {}]\n  catch error\n    side\nf()",
            10_000,
        );
        assert_cached_and_reference(
            "f = (flag, first = (padding = 1 if flag), value = (x ?= 2)) -> value\n[f(false), f(true), f(false)]",
            10_000,
        );
    }

    #[test]
    fn shared_environment_slots_preserve_closure_fallback_sharing_and_rollback() {
        let cases = [
            (
                "make = ->\n  value = 1\n  read = -> value\n  value = 2\n  read\nmake()()",
                "2",
                1_000,
            ),
            (
                "value = 40\nmake = ->\n  read = -> value\n  before = read()\n  value = 2\n  [before, read()]\nmake()",
                "[40, 2]",
                1_000,
            ),
            (
                "value = 40\nmake = (assign) ->\n  read = -> value\n  if assign\n    value = 2\n    return read()\n  read()\n[make(true), make(false)]",
                "[2, 40]",
                1_000,
            ),
            (
                "make = ->\n  value = 1\n  left = -> value\n  right = -> value\n  value = 3\n  [left, right]\nreaders = make()\n[readers[0](), readers[1]()]",
                "[3, 3]",
                1_000,
            ),
            (
                "local = 40\nmake = ->\n  read = -> local\n  try\n    [local, missing] = [1]\n  catch error\n    read()\nmake()",
                "40",
                1_000,
            ),
        ];
        for (source, expected, fuel) in cases {
            assert_cached_and_reference(source, fuel);
            assert_eq!(Context::new().eval(source).unwrap().to_string(), expected);
        }

        assert_cached_and_reference(
            "make = ->\n  value = 1\n  fail = -> missing + value\n  fail\nmake()()",
            1_000,
        );
        assert_cached_and_reference(
            "make = (limit) ->\n  value = 0\n  read = -> value\n  while value < limit then value++\n  read\nmake(1000)()",
            50,
        );

        let program = Engine::new()
            .compile_program(
                "make = ->\n  value = 0\n  read = -> value\n  value++\n  read\nmake()()",
            )
            .unwrap();
        let Constant::Function { chunk, .. } = program
            .0
            .chunk
            .constants
            .iter()
            .find(|constant| {
                matches!(
                    constant,
                    Constant::Function { chunk, .. }
                        if chunk.code.iter().any(|instruction| matches!(instruction, Instruction::MakeFunction(_) | Instruction::MakeBoundFunction(_)))
                )
            })
            .expect("capturing function template")
        else {
            unreachable!("filtered to function constants")
        };
        let slots = program
            .0
            .execution_plan
            .as_ref()
            .and_then(|plan| plan.slots(chunk))
            .unwrap();
        assert!(!slots.isolated_frame);
        assert!(slots.shared_environment);
        assert!(
            slots
                .local_names
                .iter()
                .any(|name| name.as_ref() == "value")
        );
    }

    #[test]
    fn binding_slots_preserve_errors_labels_fuel_and_raw_chunks() {
        assert_cached_and_reference("known = 1\nknown + missing", 100);
        assert_cached_and_reference("i = 0\nloop\n  i++", 50);
        assert_cached_and_reference("[left, right] = [1]", 100);
        assert_cached_and_reference(
            "increment = (value) -> value + 1\nrun = (limit) ->\n  value = 0\n  while value < limit then value = increment(value)\n  value\nrun(100)",
            50,
        );
        assert_cached_and_reference(
            "fail = -> missing\nrun = (value) -> fail() + value\nrun(1)",
            100,
        );

        let chunk = Engine::new().compile("value = 1\nvalue + 1").unwrap();
        let raw = Program::from(chunk);
        assert!(raw.0.execution_plan.is_none());
        assert_eq!(Context::new().run_program(&raw).unwrap().to_string(), "2");
    }

    #[test]
    fn retained_functions_keep_their_program_binding_plan_across_eval() {
        let engine = Engine::new();
        let define = engine
            .compile_program("base = 40\nadd = (value) -> value + base\nadd")
            .unwrap();
        let call = engine.compile_program("base = 41\nadd(1)").unwrap();
        let define_reference = define.without_binding_slots();
        let call_reference = call.without_binding_slots();

        let mut optimized = Context::new();
        let mut reference = Context::new();
        optimized.run_program(&define).unwrap();
        reference.run_program(&define_reference).unwrap();
        assert_result_equal(
            optimized.run_program(&call),
            reference.run_program(&call_reference),
        );
        assert_eq!(optimized.last_execution(), reference.last_execution());
        assert_eq!(optimized.get_global("base").unwrap().to_string(), "41");
        assert_eq!(reference.get_global("base").unwrap().to_string(), "41");

        let define = engine
            .compile_program(
                "make = ->\n  value = 1\n  read = -> value\n  value = 2\n  read\nreader = make()\nreader",
            )
            .unwrap();
        let call = engine.compile_program("reader()").unwrap();
        let define_reference = define.without_binding_slots();
        let call_reference = call.without_binding_slots();
        let mut optimized = Context::new();
        let mut reference = Context::new();
        optimized.run_program(&define).unwrap();
        reference.run_program(&define_reference).unwrap();
        assert_result_equal(
            optimized.run_program(&call),
            reference.run_program(&call_reference),
        );
        assert_eq!(optimized.last_execution(), reference.last_execution());
        assert_eq!(
            optimized.get_global("reader").unwrap().to_string(),
            "<function>"
        );
        assert_eq!(
            reference.get_global("reader").unwrap().to_string(),
            "<function>"
        );
    }

    #[test]
    fn shared_program_binding_slots_are_guarded_across_context_layouts() {
        let optimized = Engine::new()
            .compile_program(
                "f = (flag) ->\n  extra = 1 if flag\n  local = 2\n  extra ?= 0\n  local + extra\n[f(flag), x]",
            )
            .unwrap();
        let reference = optimized.without_binding_slots();

        let mut optimized_a = Context::new();
        optimized_a.set_global("x", Value::Number(40.));
        optimized_a.set_global("flag", Value::Bool(false));
        let mut optimized_b = Context::new();
        optimized_b.set_global("padding", Value::Nil);
        optimized_b.set_global("x", Value::Number(41.));
        optimized_b.set_global("flag", Value::Bool(true));

        let mut reference_a = Context::new();
        reference_a.set_global("x", Value::Number(40.));
        reference_a.set_global("flag", Value::Bool(false));
        let mut reference_b = Context::new();
        reference_b.set_global("padding", Value::Nil);
        reference_b.set_global("x", Value::Number(41.));
        reference_b.set_global("flag", Value::Bool(true));

        fn run_pair(
            optimized: &Program,
            reference: &Program,
            optimized_context: &mut Context,
            reference_context: &mut Context,
        ) {
            assert_result_equal(
                optimized_context.run_program(optimized),
                reference_context.run_program(reference),
            );
            assert_eq!(
                optimized_context.last_execution(),
                reference_context.last_execution()
            );
        }
        run_pair(&optimized, &reference, &mut optimized_a, &mut reference_a);
        run_pair(&optimized, &reference, &mut optimized_b, &mut reference_b);
        run_pair(&optimized, &reference, &mut optimized_a, &mut reference_a);

        let captured = Engine::new()
            .compile_program("read = (value) -> value + host\nread(1)")
            .unwrap();
        let captured_reference = captured.without_binding_slots();
        optimized_a.set_global("host", Value::Number(40.));
        optimized_b.set_global("host", Value::Number(41.));
        reference_a.set_global("host", Value::Number(40.));
        reference_b.set_global("host", Value::Number(41.));
        run_pair(
            &captured,
            &captured_reference,
            &mut optimized_a,
            &mut reference_a,
        );

        let shared = Engine::new()
            .compile_program(
                "make = (flag) ->\n  read = -> value\n  value = host if flag\n  [read(), host]\nmake(flag)",
            )
            .unwrap();
        let shared_reference = shared.without_binding_slots();
        optimized_a.set_global("value", Value::Number(40.));
        optimized_a.set_global("host", Value::Number(2.));
        optimized_a.set_global("flag", Value::Bool(false));
        optimized_b.set_global("value", Value::Number(41.));
        optimized_b.set_global("host", Value::Number(3.));
        optimized_b.set_global("flag", Value::Bool(true));
        reference_a.set_global("value", Value::Number(40.));
        reference_a.set_global("host", Value::Number(2.));
        reference_a.set_global("flag", Value::Bool(false));
        reference_b.set_global("value", Value::Number(41.));
        reference_b.set_global("host", Value::Number(3.));
        reference_b.set_global("flag", Value::Bool(true));
        run_pair(
            &shared,
            &shared_reference,
            &mut optimized_a,
            &mut reference_a,
        );
        run_pair(
            &shared,
            &shared_reference,
            &mut optimized_b,
            &mut reference_b,
        );
        run_pair(
            &shared,
            &shared_reference,
            &mut optimized_a,
            &mut reference_a,
        );
        run_pair(
            &captured,
            &captured_reference,
            &mut optimized_b,
            &mut reference_b,
        );
        run_pair(
            &captured,
            &captured_reference,
            &mut optimized_a,
            &mut reference_a,
        );
    }

    #[test]
    fn compiler_resolved_frame_slots_preserve_leaf_calls_persistence_and_failure_state() {
        assert_cached_and_reference(
            "increment = (value) -> value + 1\nsum = 0\ni = 0\nwhile i < 100\n  sum = increment(sum)\n  i++\nsum",
            10_000,
        );
        assert_cached_and_reference(
            "increment = (value) -> value + 1\nrun = (limit) ->\n  sum = 0\n  i = 0\n  while i < limit\n    sum = increment(sum)\n    i++\n  sum\nrun(100)",
            10_000,
        );
        assert_cached_and_reference(
            "factorial = (n) -> if n == 0 then 1 else n * factorial(n - 1)\nfactorial(8)",
            10_000,
        );
        assert_cached_and_reference(
            "outer = 7\nread = (condition) ->\n  if condition then outer = 9\n  outer\n[read(false), read(true), outer]",
            1_000,
        );
        let leaf = Engine::new()
            .compile_program("increment = (value) -> value + 1\nincrement(41)")
            .unwrap();
        let Constant::Function {
            params,
            required,
            rest,
            chunk,
            ..
        } = &leaf.0.chunk.constants[0]
        else {
            panic!("first constant is the leaf function template");
        };
        let leaf_slots = leaf
            .0
            .execution_plan
            .as_ref()
            .and_then(|plan| plan.slots(chunk))
            .unwrap();
        assert!(
            leaf_slots
                .fast_parameter_slots(params, *required, rest.as_deref())
                .is_some()
        );

        let caller = Engine::new()
            .compile_program(
                "increment = (value) -> value + 1\nrun = (limit) -> increment(limit)\nrun(41)",
            )
            .unwrap();
        let Constant::Function {
            params,
            required,
            rest,
            chunk,
            ..
        } = caller
            .0
            .chunk
            .constants
            .iter()
            .find(|constant| {
                matches!(
                    constant,
                    Constant::Function { chunk, .. }
                        if chunk.code.iter().any(|instruction| matches!(instruction, Instruction::Call(_)))
                )
            })
            .expect("caller function template")
        else {
            unreachable!("filtered to function constants")
        };
        let caller_slots = caller
            .0
            .execution_plan
            .as_ref()
            .and_then(|plan| plan.slots(chunk))
            .unwrap();
        assert!(caller_slots.isolated_frame);
        assert!(
            caller_slots
                .fast_parameter_slots(params, *required, rest.as_deref())
                .is_some()
        );

        let repeated = Engine::new()
            .compile_program("i ?= 0\nlimit = i + 3\nwhile i < limit then i++\ni")
            .unwrap();
        let repeated_reference = repeated.without_binding_slots();
        let slots = repeated
            .0
            .execution_plan
            .as_ref()
            .and_then(|plan| plan.slots(&repeated.0.chunk))
            .unwrap();
        assert!(slots.isolated_frame);
        let mut optimized_context = Context::new();
        let mut reference_context = Context::new();
        for expected in ["3", "6"] {
            assert_eq!(
                optimized_context
                    .run_program(&repeated)
                    .unwrap()
                    .to_string(),
                expected
            );
            assert_eq!(
                reference_context
                    .run_program(&repeated_reference)
                    .unwrap()
                    .to_string(),
                expected
            );
            assert_eq!(
                optimized_context.last_execution(),
                reference_context.last_execution()
            );
        }

        let exhausted = Engine::new()
            .compile_program(
                "spin = (limit) ->\n  i = 0\n  while i < limit\n    i++\n  i\nspin(1000)",
            )
            .unwrap();
        let exhausted_reference = exhausted.without_binding_slots();
        let mut optimized_context = Context::new().with_fuel(50);
        let mut reference_context = Context::new().with_fuel(50);
        assert_result_equal(
            optimized_context.run_program(&exhausted),
            reference_context.run_program(&exhausted_reference),
        );
        assert_eq!(
            optimized_context.last_execution(),
            reference_context.last_execution()
        );
        assert!(optimized_context.get_global("i").is_none());
        assert!(reference_context.get_global("i").is_none());
    }
}
