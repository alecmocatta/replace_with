# replace_with

[![Crates.io](https://img.shields.io/crates/v/replace_with.svg?maxAge=86400&)](https://crates.io/crates/replace_with)
[![Apache-2.0 licensed](https://img.shields.io/crates/l/replace_with.svg?maxAge=2592000&)](LICENSE.txt)
[![Build Status](https://travis-ci.com/alecmocatta/replace_with.svg?branch=master)](https://travis-ci.com/alecmocatta/replace_with)

[Docs](https://docs.rs/replace_with/0.1.1)

Temporarily take ownership of a value at a mutable location, and replace it with a new value based on the old one.

This crate provides the function [`replace_with()`](https://docs.rs/replace_with/0.1.1/replace_with/fn.replace_with.html), which is like [`std::mem::replace()`](https://doc.rust-lang.org/std/mem/fn.replace.html) except it allows the replacement value to be mapped from the original value.

See [RFC 1736](https://github.com/rust-lang/rfcs/pull/1736) for a lot of discussion as to its merits. It was never merged, and the desired ability to temporarily move out of `&mut T` doesn't exist yet, so this crate is my interim solution.

It's very akin to [`take_mut`](https://github.com/Sgeo/take_mut), though uses `Drop` instead of [`std::panic::catch_unwind()`](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html) to react to unwinding, which avoids the optimisation barrier of calling the `extern "C" __rust_maybe_catch_panic()`. As such it's up to ∞x faster. It's also aesthetically a little prettier (I think).

## Example

Consider this motivating example:

```rust
enum States {
	A(String),
	B(String),
}

impl States {
	fn poll(&mut self) {
		// error[E0507]: cannot move out of borrowed content
		*self = match *self {
		//            ^^^^^ cannot move out of borrowed content
			States::A(a) => States::B(a),
			States::B(a) => States::A(a),
		};
	}
}
```

Depending on context this can be quite tricky to work around. With this crate, however:

```rust
enum States {
	A(String),
	B(String),
}

impl States {
	fn poll(&mut self) {
		replace_with_or_abort(self, |self_| match self_ {
			States::A(a) => States::B(a),
			States::B(a) => States::A(a),
		});
	}
}
```

Huzzah!

## License
Licensed under Apache License, Version 2.0, ([LICENSE.txt](LICENSE.txt) or http://www.apache.org/licenses/LICENSE-2.0).

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be licensed as above, without any additional terms or conditions.
