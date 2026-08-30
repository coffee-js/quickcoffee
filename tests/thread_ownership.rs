use quickcoffee::{CancellationToken, Context, ModulePackage, Program, Runtime};

// Stable Rust has no direct negative trait-bound syntax. This assertion gives
// the compiler one unambiguous implementation only while `$type: !$trait`.
// If the type starts implementing the trait, the inferred marker becomes
// ambiguous and this integration target fails to compile on both stable/MSRV.
macro_rules! assert_not_impl {
    ($type:ty => $trait:path) => {
        const _: fn() = || {
            trait AmbiguousIfImpl<Marker> {
                fn assert_not_impl() {}
            }

            impl<T: ?Sized> AmbiguousIfImpl<()> for T {}

            struct Invalid;
            impl<T: ?Sized + $trait> AmbiguousIfImpl<Invalid> for T {}

            let _ = <$type as AmbiguousIfImpl<_>>::assert_not_impl;
        };
    };
}

assert_not_impl!(Runtime => Send);
assert_not_impl!(Runtime => Sync);
assert_not_impl!(Program => Send);
assert_not_impl!(Program => Sync);
assert_not_impl!(ModulePackage => Send);
assert_not_impl!(ModulePackage => Sync);
assert_not_impl!(Context => Send);
assert_not_impl!(Context => Sync);

#[test]
fn cancellation_token_remains_send_and_sync() {
    fn require_send_sync<T: Send + Sync>() {}
    require_send_sync::<CancellationToken>();
}
