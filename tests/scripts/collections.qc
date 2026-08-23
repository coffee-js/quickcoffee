# test: map spread, destructuring, and Unicode indexing remain strict
defaults = {theme: 'light', retries: 1}
config = {...defaults, retries: 3}
{theme, retries} = config
theme == 'light' and retries == 3 and 'a☕中'[1] == '☕'
