//! Temporarily take ownership of a value at a mutable location, and replace it with a new value
//! based on the old one.
//!
//! **[Crates.io](https://crates.io/crates/replace_with) │ [Repo](https://github.com/alecmocatta/replace_with)**
//!
//! This crate provides the function [`replace_with()`], which is like [`std::mem::replace()`]
//! except it allows the replacement value to be mapped from the original value.
//!
//! See [RFC 1736](https://github.com/rust-lang/rfcs/pull/1736) for a lot of discussion as to its
//! merits. It was never merged, and the desired ability to temporarily move out of `&mut T` doesn't
//! exist yet, so this crate is my interim solution.
//!
//! It's very akin to [`take_mut`](https://github.com/Sgeo/take_mut), though uses `Drop` instead of
//! [`std::panic::catch_unwind()`] to react to unwinding, which avoids the optimisation barrier of
//! calling the `extern "C" __rust_maybe_catch_panic()`. As such it's up to ∞x faster. The API also
//! attempts to make slightly more explicit the behavior on panic – [`replace_with()`] accepts two
//! closures such that aborting in the "standard case" where the mapping closure (`FnOnce(T) -> T`)
//! panics (as [`take_mut::take()`](https://docs.rs/take_mut/0.2.2/take_mut/fn.take.html) does) is
//! avoided. If the second closure (`FnOnce() -> T`) panics, however, then it does indeed abort.
//! The "abort on first panic" behaviour is available with [`replace_with_or_abort()`].
//!
//! # Example
//!
//! Consider this motivating example:
//!
//! ```compile_fail
//! # use replace_with::*;
//! enum States {
//! 	A(String),
//! 	B(String),
//! }
//!
//! impl States {
//! 	fn poll(&mut self) {
//! 		// error[E0507]: cannot move out of borrowed content
//! 		*self = match *self {
//! 		//            ^^^^^ cannot move out of borrowed content
//! 			States::A(a) => States::B(a),
//! 			States::B(a) => States::A(a),
//! 		};
//! 	}
//! }
//! ```
//!
//! Depending on context this can be quite tricky to work around. With this crate, however:
//!
//! ```
//! # use replace_with::*;
//! enum States {
//! 	A(String),
//! 	B(String),
//! }
//!
//! # #[cfg(any(feature = "std", feature = "nightly"))]
//! impl States {
//! 	fn poll(&mut self) {
//! 		replace_with_or_abort(self, |self_| match self_ {
//! 			States::A(a) => States::B(a),
//! 			States::B(a) => States::A(a),
//! 		});
//! 	}
//! }
//! ```
//!
//! Huzzah!

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(
	all(not(feature = "std"), feature = "nightly"),
	feature(core_intrinsics)
)]
#![doc(html_root_url = "https://docs.rs/replace_with/0.1.2")]

#[cfg(not(feature = "std"))]
extern crate core as std;

use std::{mem, ptr};

struct CatchUnwind<F: FnOnce()>(mem::ManuallyDrop<F>);
impl<F: FnOnce()> Drop for CatchUnwind<F> {
	#[inline(always)]
	fn drop(&mut self) {
		(unsafe { ptr::read(&*self.0) })();
	}
}

#[inline(always)]
fn catch_unwind<F: FnOnce() -> T, T, P: FnOnce()>(f: F, p: P) -> T {
	let x = CatchUnwind(mem::ManuallyDrop::new(p));
	let t = f();
	let _ = unsafe { ptr::read(&*x.0) };
	mem::forget(x);
	t
}

/// Temporarily takes ownership of a value at a mutable location, and replace it with a new value
/// based on the old one.
///
/// We move out of the reference temporarily, to apply a closure `f`, returning a new value, which
/// is then placed at the original value's location.
///
/// # An important note
///
/// On panic (or to be more precise, unwinding) of the closure `f`, `default` will be called to
/// provide a replacement value. `default` should not panic – doing so will constitute a double
/// panic and will most likely abort the process.
///
/// # Example
///
/// ```
/// # use replace_with::*;
/// enum States {
/// 	A(String),
/// 	B(String),
/// }
///
/// impl States {
/// 	fn poll(&mut self) {
/// 		replace_with(
/// 			self,
/// 			|| States::A(String::new()),
/// 			|self_| match self_ {
/// 				States::A(a) => States::B(a),
/// 				States::B(a) => States::A(a),
/// 			},
/// 		);
/// 	}
/// }
/// ```
#[inline]
pub fn replace_with<T, D: FnOnce() -> T, F: FnOnce(T) -> T>(dest: &mut T, default: D, f: F) {
	unsafe {
		let t = ptr::read(dest);
		let t = catch_unwind(move || f(t), || ptr::write(dest, default()));
		ptr::write(dest, t);
	}
}

/// Temporarily takes ownership of a value at a mutable location, and replace it with a new value
/// based on the old one. Replaces with [`Default::default()`] on panic.
///
/// We move out of the reference temporarily, to apply a closure `f`, returning a new value, which
/// is then placed at the original value's location.
///
/// # An important note
///
/// On panic (or to be more precise, unwinding) of the closure `f`, `T::default()` will be called to
/// provide a replacement value. `T::default()` should not panic – doing so will constitute a double
/// panic and will most likely abort the process.
///
/// Equivalent to `replace_with(dest, T::default, f)`.
///
/// Differs from `*dest = mem::replace(dest, Default::default())` in that `Default::default()` will
/// only be called on panic.
///
/// # Example
///
/// ```
/// # use replace_with::*;
/// enum States {
/// 	A(String),
/// 	B(String),
/// }
///
/// impl Default for States {
/// 	fn default() -> Self {
/// 		States::A(String::new())
/// 	}
/// }
///
/// impl States {
/// 	fn poll(&mut self) {
/// 		replace_with_or_default(self, |self_| match self_ {
/// 			States::A(a) => States::B(a),
/// 			States::B(a) => States::A(a),
/// 		});
/// 	}
/// }
/// ```
#[inline]
pub fn replace_with_or_default<T: Default, F: FnOnce(T) -> T>(dest: &mut T, f: F) {
	replace_with(dest, T::default, f);
}

/// Temporarily takes ownership of a value at a mutable location, and replace it with a new value
/// based on the old one. Aborts on panic.
///
/// We move out of the reference temporarily, to apply a closure `f`, returning a new value, which
/// is then placed at the original value's location.
///
/// # An important note
///
/// On panic (or to be more precise, unwinding) of the closure `f`, the process will **abort** to
/// avoid returning control while `dest` is in a potentially invalid state.
///
/// If this behaviour is undesirable, use [replace_with] or [replace_with_or_default].
///
/// Equivalent to `replace_with(dest, || process::abort(), f)`.
///
/// # Example
///
/// ```
/// # use replace_with::*;
/// enum States {
/// 	A(String),
/// 	B(String),
/// }
///
/// # #[cfg(any(feature = "std", feature = "nightly"))]
/// impl States {
/// 	fn poll(&mut self) {
/// 		replace_with_or_abort(self, |self_| match self_ {
/// 			States::A(a) => States::B(a),
/// 			States::B(a) => States::A(a),
/// 		});
/// 	}
/// }
/// ```
#[inline]
#[cfg(feature = "std")]
pub fn replace_with_or_abort<T, F: FnOnce(T) -> T>(dest: &mut T, f: F) {
	replace_with(dest, || std::process::abort(), f);
}

#[inline]
#[cfg(all(not(feature = "std"), feature = "nightly"))]
pub fn replace_with_or_abort<T, F: FnOnce(T) -> T>(dest: &mut T, f: F) {
	replace_with(dest, || unsafe { std::intrinsics::abort() }, f);
}

/// Temporarily takes ownership of a value at a mutable location, and replace it with a new value
/// based on the old one. Aborts on panic.
///
/// We move out of the reference temporarily, to apply a closure `f`, returning a new value, which
/// is then placed at the original value's location.
///
/// # An important note
///
/// On panic (or to be more precise, unwinding) of the closure `f`, the process will **abort** to
/// avoid returning control while `dest` is in a potentially invalid state.
///
/// Unlike for `replace_with_or_abort()` users of `replace_with_or_abort_unchecked()` are expected
/// to have `features = ["panic_abort", …]` defined in `Cargo.toml`
/// and `panic = "abort"` defined in their profile for it to behave semantically correct:
///
/// ```toml
/// # Cargo.toml
///
/// [profile.debug]
/// panic = "abort"
///
/// [profile.release]
/// panic = "abort"
/// ```
///
/// **Word of caution:** It is crucial to only ever use this function having defined `panic = "abort"`,
/// or else bad things may happen. It's *up to you* to uphold this invariant!
///
/// If this behaviour is undesirable, use [replace_with] or [replace_with_or_default].
///
/// Equivalent to `replace_with(dest, || process::abort(), f)`.
///
/// # Example
///
/// ```
/// # use replace_with::*;
/// enum States {
/// 	A(String),
/// 	B(String),
/// }
///
/// impl States {
/// 	fn poll(&mut self) {
/// 		unsafe {
/// 			replace_with_or_abort_unchecked(self, |self_| match self_ {
/// 				States::A(a) => States::B(a),
///	 				States::B(a) => States::A(a),
/// 			});
/// 		}
/// 	}
/// }
/// ```
///
#[inline]
#[cfg(feature = "panic_abort")]
pub unsafe fn replace_with_or_abort_unchecked<T, F: FnOnce(T) -> T>(dest: &mut T, f: F) {
	ptr::write(dest, f(ptr::read(dest)));
}

#[cfg(test)]
mod test {
	// These functions copied from https://github.com/Sgeo/take_mut/blob/1bd70d842c6febcd16ec1fe3a954a84032b89f52/src/lib.rs#L102-L147

	// Copyright (c) 2016 Sgeo

	// Permission is hereby granted, free of charge, to any person obtaining a copy
	// of this software and associated documentation files (the "Software"), to deal
	// in the Software without restriction, including without limitation the rights
	// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
	// copies of the Software, and to permit persons to whom the Software is
	// furnished to do so, subject to the following conditions:

	// The above copyright notice and this permission notice shall be included in all
	// copies or substantial portions of the Software.

	// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
	// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
	// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
	// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
	// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
	// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
	// SOFTWARE.

	use super::*;

	#[test]
	fn it_works_recover() {
		#[derive(PartialEq, Eq, Debug)]
		enum Foo {
			A,
			B,
		};
		impl Drop for Foo {
			#[cfg(feature = "std")]
			fn drop(&mut self) {
				match *self {
					Foo::A => println!("Foo::A dropped"),
					Foo::B => println!("Foo::B dropped"),
				}
			}

			#[cfg(not(feature = "std"))]
			fn drop(&mut self) {
				match *self {
					Foo::A => (),
					Foo::B => (),
				}
			}
		}
		let mut quax = Foo::A;
		replace_with(
			&mut quax,
			|| Foo::A,
			|f| {
				drop(f);
				Foo::B
			},
		);
		assert_eq!(&quax, &Foo::B);
	}

	#[cfg(feature = "std")]
	#[test]
	fn it_works_recover_panic() {
		use std::panic;

		#[derive(PartialEq, Eq, Debug)]
		enum Foo {
			A,
			B,
			C,
		};
		impl Drop for Foo {
			fn drop(&mut self) {
				match *self {
					Foo::A => println!("Foo::A dropped"),
					Foo::B => println!("Foo::B dropped"),
					Foo::C => println!("Foo::C dropped"),
				}
			}
		}
		let mut quax = Foo::A;

		let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
			replace_with(
				&mut quax,
				|| Foo::C,
				|f| {
					drop(f);
					panic!("panic");
					#[allow(unreachable_code)]
					Foo::B
				},
			);
		}));

		assert!(res.is_err());
		assert_eq!(&quax, &Foo::C);
	}
}
