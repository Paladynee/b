// This is a module that facilitates Crust-style programming - https://github.com/tsoding/crust
use crate::crust::libc::*;
use core::panic::PanicInfo;
use core::ffi::*;

#[macro_export]
macro_rules! c {
    ($l:expr) => {
        concat!($l, "\0").as_ptr() as *const c_char
    }
}

#[macro_export]
macro_rules! enum_with_order {
    (
        $( #[$attr:meta $($attr_args:tt)*] )*
        enum $Ident:ident {
            $(
                $Item:ident $(= $init:expr)?
            ),* $(,)?
        }
    ) => {
        $( #[$attr $($attr_args)*] )*
        pub enum $Ident {
            $($Item $(= $init)?),*
        }

        impl $Ident {
            /*
                Explanation:
                Rust enums have the ability to specify their variants' values, like `A` in this enum:
                enum Either {
                    A = 1,
                    B,
                }
                in order to make the ordered slice of variants, we need compile time buffers of unknown sizes.
                we take this const fn approach from crates like `const_str`:
                we have a const function with a const-generic array width (named order_variants_properly) to have an actual array
                on the const stack which we can modify freely and return by-value. this is all it does.
            */
            const __ORDER_AND_NAMES_SLICES: (*const [$Ident], *const [*const c_char]) = {
                use $Ident::*;
                use $crate::fighting_consteval::*;
                #[allow(unused_imports)]
                use $crate::c;
                // we assert that $Ident must be Copy here (as Crust requires us to do so!)
                const fn _assert_copy<T: Copy>() {}
                const _: () = _assert_copy::<$Ident>(); // your enum must derive Copy to have an ordered slice!
                // this is the slice of declarations in declaration order. declaration order does not mean
                // order of appearance in the ORDER_SLICE, as rust allows explicit discriminants.
                const DECLS_AMOUNT: usize = UNORDERED_DECLS.len();
                const UNORDERED_DECLS: *const [($Ident, *const c_char)] = &[
                    $(
                        ($Item, c!(stringify!($Item)))
                    ),*
                ];
                // this is the slice of declarations that have a specified enum discriminant requirement.
                // the order of elements inside this slice doesn't really matter.
                // we don't have to worry about clashing requirements as the enum declaration itself handles that for us.
                const AMOUNT_SPECIFIED: usize = SPECIFIED_DISCRIMINANTS.len();
                const SPECIFIED_DISCRIMINANTS: *const [($Ident, *const c_char, u128)] = &[
                    $( $(
                        ($Item, c!(stringify!($Item)), $init), // negative discriminants are not supported, as Self::ORDER_SLICE would need to go backwards.
                    )? )*
                ];
                // we pass the unordered declarations and the discriminant requirements to `order_decls_properly`,
                // which handles the discriminant resolution in a const fn that is fully evaluated at const.
                const RES: ([$Ident; DECLS_AMOUNT], [*const c_char; DECLS_AMOUNT]) = unsafe {
                    #[allow(unused_imports)]
                    use OrderDeclsError::*;
                    match const { order_decls_properly::<$Ident, DECLS_AMOUNT, AMOUNT_SPECIFIED>(
                        &*mkarray::<_, DECLS_AMOUNT>(UNORDERED_DECLS),
                        &*mkarray::<_, AMOUNT_SPECIFIED>(SPECIFIED_DISCRIMINANTS)
                    ) } {
                        Ok(v) => v,
                        Err(OrderDeclsError::RanOutOfDecls) =>
                            panic!("enum_with_order: failed to order enum variants properly.\n\tthis is likely due to discriminant requirements leaving holes in the resulting ORDER_SLICE, which is not supported"),
                        Err(OrderDeclsError::FinalSliceMissingEntries) =>
                            panic!("enum_with_order: critical sanity check failed at compile time. this is a bug.\n\tthere were entries in your declaration that did not end up in the resulting `Self::ORDER_SLICE`"),
                    }
                };
                // as constants don't allow destructuring (as in `const (ORDER, NAMES) = ...;`), we unpack RES
                // manually and "return" it as the constant.
                (
                    &RES.0,
                    &RES.1
                )
            };
            pub const ORDER_SLICE: *const [$Ident] = $Ident::__ORDER_AND_NAMES_SLICES.0;
            pub const NAMES_SLICE: *const [*const c_char] = $Ident::__ORDER_AND_NAMES_SLICES.1;
            pub const VARIANT_COUNT: usize = unsafe { (&*$Ident::ORDER_SLICE).len() };
        }
    };
}

pub unsafe fn slice_contains<Value: PartialEq>(slice: *const [Value], needle: *const Value) -> bool {
    for i in 0..slice.len() {
        if (*slice)[i] == *needle {
            return true
        }
    }
    false
}

pub unsafe fn assoc_lookup_cstr_mut<Value>(assoc: *mut [(*const c_char, Value)], needle: *const c_char) -> Option<*mut Value> {
    for i in 0..assoc.len() {
        if strcmp((*assoc)[i].0, needle) == 0 {
            return Some(&mut (*assoc)[i].1);
        }
    }
    None
}

pub unsafe fn assoc_lookup_cstr<Value>(assoc: *const [(*const c_char, Value)], needle: *const c_char) -> Option<*const Value> {
    for i in 0..assoc.len() {
        if strcmp((*assoc)[i].0, needle) == 0 {
            return Some(&(*assoc)[i].1);
        }
    }
    None
}

pub unsafe fn assoc_lookup_mut<Key, Value>(assoc: *mut [(Key, Value)], needle: *const Key) -> Option<*mut Value>
where Key: PartialEq
{
    for i in 0..assoc.len() {
        if (*assoc)[i].0 == *needle {
            return Some(&mut (*assoc)[i].1);
        }
    }
    None
}

pub unsafe fn assoc_lookup<Key, Value>(assoc: *const [(Key, Value)], needle: *const Key) -> Option<*const Value>
where Key: PartialEq
{
    for i in 0..assoc.len() {
        if (*assoc)[i].0 == *needle {
            return Some(&(*assoc)[i].1);
        }
    }
    None
}

#[macro_use]
pub mod libc {
    use core::ffi::*;

    pub type FILE = c_void;

    extern "C" {
        #[link_name = "get_stdin"]
        pub fn stdin() -> *mut FILE;
        #[link_name = "get_stdout"]
        pub fn stdout() -> *mut FILE;
        #[link_name = "get_stderr"]
        pub fn stderr() -> *mut FILE;
        pub fn fopen(pathname: *const c_char, mode: *const c_char) -> *mut FILE;
        pub fn fclose(stream: *mut FILE) -> c_int;
        pub fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int;
        pub fn strchr(s: *const c_char, c: c_int) -> *const c_char;
        pub fn strrchr(s: *const c_char, c: c_int) -> *const c_char;
        pub fn strlen(s: *const c_char) -> usize;
        pub fn strtoull(nptr: *const c_char, endptr: *mut*mut c_char, base: c_int) -> c_ulonglong;
        pub fn fwrite(ptr: *const c_void, size: usize, nmemb: usize, stream: *mut FILE) -> usize;

        pub fn abort() -> !;
        pub fn strdup(s: *const c_char) -> *mut c_char;
        pub fn strncpy(dst: *mut c_char, src: *const c_char, dsize: usize) -> *mut c_char;
        pub fn printf(fmt: *const c_char, ...) -> c_int;
        pub fn fprintf(stream: *mut FILE, fmt: *const c_char, ...) -> c_int;
        pub fn memset(dest: *mut c_void, byte: c_int, size: usize) -> c_int;
        pub fn isspace(c: c_int) -> c_int;
        pub fn isalpha(c: c_int) -> c_int;
        pub fn isalnum(c: c_int) -> c_int;
        pub fn isdigit(c: c_int) -> c_int;
        pub fn isprint(c: c_int) -> c_int;
        pub fn tolower(c: c_int) -> c_int;
        pub fn toupper(c: c_int) -> c_int;
        pub fn qsort(base: *mut c_void, nmemb: usize, size: usize, compar: unsafe extern "C" fn(*const c_void, *const c_void) -> c_int);
        pub fn dirname(path: *const c_char) -> *const c_char;
    }

    // count is the amount of items, not bytes
    pub unsafe fn realloc_items<T>(ptr: *mut T, count: usize) -> *mut T {
        extern "C" {
            #[link_name = "realloc"]
            fn realloc_raw(ptr: *mut c_void, size: usize) -> *mut c_void;
        }
        realloc_raw(ptr as *mut c_void, size_of::<T>()*count) as *mut T
    }

    pub unsafe fn free<T>(ptr: *mut T) {
        extern "C" {
            #[link_name = "free"]
            fn free_raw(ptr: *mut c_void);
        }
        free_raw(ptr as *mut c_void);
    }
}

pub unsafe extern "C" fn compar_cstr(a: *const c_void, b: *const c_void) -> c_int {
    strcmp(*(a as *const *const c_char), *(b as *const *const c_char))
}

#[panic_handler]
pub unsafe fn panic_handler(info: &PanicInfo) -> ! {
    // TODO: What's the best way to implement the panic handler within the Crust spirit
    //   PanicInfo must be passed by reference.
    if let Some(location) = info.location() {
        fprintf(stderr(), c!("%.*s:%d: "), location.file().len(), location.file().as_ptr(), location.line());
    }
    fprintf(stderr(), c!("panicked"));
    if let Some(message) = info.message().as_str() {
        fprintf(stderr(), c!(": %.*s"), message.len(), message.as_ptr());
    }
    fprintf(stderr(), c!("\n"));
    abort()
}

#[export_name="main"]
pub unsafe extern "C" fn crust_entry_point(argc: i32, argv: *mut*mut c_char) -> i32 {
    match crate::main(argc, argv) {
        Some(()) => 0,
        None => 1,
    }
}

#[no_mangle]
pub unsafe fn rust_eh_personality() {
    // TODO: Research more what this is used for. Maybe we could put something useful in here.
}
