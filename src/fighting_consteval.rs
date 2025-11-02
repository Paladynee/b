use core::mem::MaybeUninit;
use core::mem::size_of;
use core::ffi::*;

#[track_caller]
pub const unsafe fn mkarray<T, const N: usize>(slice: *const [T]) -> *const [T; N] {
    if slice.len() != N {
        panic!("slice length does not match array length");
    }
    &*(slice as *const [T; N])
}

pub const unsafe fn mkslice<T, const N: usize>(array: *const [T; N]) -> *const [T] {
    core::ptr::slice_from_raw_parts(array.cast::<T>(), N)
}

pub const unsafe fn bitwise_partialeq<T>(lhs: *const T, rhs: *const T) -> bool {
    let lhs_bytes = lhs as *const u8;
    let rhs_bytes = rhs as *const u8;
    let size = size_of::<T>();
    let mut i = 0;
    while i < size {
        if *lhs_bytes.add(i) != *rhs_bytes.add(i) {
            return false;
        }
        i += 1;
    }
    true
}

pub const unsafe fn const_slice_bitwise_contains<T>(
    haystack: *const [T],
    needle: *const T,
) -> bool {
    let mut i = 0;
    while i < haystack.len() {
        let indexed = slice_index(haystack, i);
        if bitwise_partialeq(indexed, needle) {
            return true;
        }
        i += 1;
    }
    false
}

#[track_caller]
pub const unsafe fn slice_index<T>(slice: *const [T], index: usize) -> *const T {
    assert!(index < slice.len(), "slice index out of bounds");
    slice.cast::<T>().add(index)
}

#[track_caller]
pub const unsafe fn slice_index_mut<T>(slice: *mut [T], index: usize) -> *mut T {
    assert!(index < slice.len(), "slice index out of bounds");
    slice.cast::<T>().add(index)
}

pub const unsafe fn reduce_slice_to_array_of_field_copied<
    T,
    const LEN_SPECIFIED: usize
>(
    slice: *const [(T, *const c_char, u128)]
) -> [T; LEN_SPECIFIED] {
    let mut buf: [MaybeUninit<T>; LEN_SPECIFIED] =
        [const { MaybeUninit::uninit() }; LEN_SPECIFIED];
    let mut i = 0;
    while i < LEN_SPECIFIED {
        let indexed = slice_index(slice, i);
        buf[i].write((&raw const (*indexed).0).read());
        i += 1;
    }
    buf.as_ptr().cast::<[T; LEN_SPECIFIED]>().read()
}

pub const unsafe fn get_unspecified_from_unordered_and_specified_decls<
    T,
    const LEN_TOTAL: usize,
    const LEN_SPECIFIED: usize
>(
    total_unordered: *const [(T, *const c_char      ); LEN_TOTAL    ],
    specified      : *const [(T, *const c_char, u128); LEN_SPECIFIED],
) -> Result<(
    [MaybeUninit<(T, *const c_char)>; LEN_TOTAL],
    usize
), OrderDeclsError> {
    // N.B. Uninit([ T ]) and not [ Uninit(T) ]
    // we use Uninit as a replacement for ManuallyDrop, so we can let the const evaluator
    // rest assured knowing that whether T: Drop or not is irrelevant.
    // the arrays are always in a fully initialized state.
    let specified_array: MaybeUninit<[T                         ; LEN_SPECIFIED]> = MaybeUninit::new(reduce_slice_to_array_of_field_copied(specified));
    let mut unspecified: MaybeUninit<[Option<(T, *const c_char)>; LEN_TOTAL    ]> = MaybeUninit::new([const { None }; LEN_TOTAL]);
    let mut unspecified_len = 0;
    let mut i = 0;

    while i < LEN_TOTAL {
        let in_question = slice_index(total_unordered, i);
        if !const_slice_bitwise_contains(
            mkslice::<T, LEN_SPECIFIED>(specified_array.as_ptr()),
            &raw const (*in_question).0
        ) {
            core::ptr::write(
                slice_index_mut(unspecified.as_mut_ptr(), unspecified_len),
                Some(in_question.read())
            );
            unspecified_len += 1;
        }
        i += 1;
    }

    let mut final_buf: [MaybeUninit<(T, *const c_char)>; LEN_TOTAL] = [const { MaybeUninit::uninit() }; LEN_TOTAL];
    let mut j = 0;
    while j < unspecified_len {
        let val = slice_index(unspecified.as_ptr(), j);
        match &*val {
            &Some(ref t) => 
                final_buf[j].write((&raw const *t).read()),
            &None => return Err(OrderDeclsError::RanOutOfDecls),
        };
        j += 1;
    }

    Ok((final_buf, unspecified_len))
}

#[derive(Clone, Copy)]
pub enum OrderDeclsError {
    RanOutOfDecls,
    FinalSliceMissingEntries,
}

pub const unsafe fn order_decls_properly<T, const LEN_TOTAL: usize, const LEN_SPECIFIED: usize>(
    total_unordered: *const [(T, *const c_char      ); LEN_TOTAL    ],
    specified:       *const [(T, *const c_char, u128); LEN_SPECIFIED],
) -> Result<(
    [T            ; LEN_TOTAL],
    [*const c_char; LEN_TOTAL]
), OrderDeclsError> {
    // [D,   D,   D,   D,   D,   D,   D,   D,   D,   D,   D   ] total_unordered
    // [D=5, D=3                                              ] specified
    // [tu0, tu1, tu2, sp1, tu3, sp0, tu4, tu5, tu6, tu7, tu8 ] result 
    let res = match get_unspecified_from_unordered_and_specified_decls::<
        T, LEN_TOTAL, LEN_SPECIFIED
    >(
        total_unordered, specified
    ) {
        Ok(t) => t,
        Err(e) => return Err(e),
    };
    
    // N.B. [ Uninit(T) ] and not Uninit([ T ])
    let unspecified:     [MaybeUninit<(T, *const c_char)>; LEN_TOTAL] = res.0;
    let unspecified_len: usize = res.1;
    // N.B. [ Uninit(T) ] and not Uninit([ T ])
    // these are incrementally initialized arrays. we will be returning these from the function.
    let mut result_t   : [MaybeUninit<T>                 ; LEN_TOTAL] = [const { MaybeUninit::uninit() }; LEN_TOTAL];
    let mut result_char: [MaybeUninit<*const c_char>     ; LEN_TOTAL] = [const { MaybeUninit::uninit() }; LEN_TOTAL];
    let mut unspecified_iter = 0;
    let mut i = 0;
    while i < LEN_TOTAL {
        if let Some(found) = 'a: {
            // specified.iter().find(|(_, _, discrim)| *discrim == i as u128)
            let mut j = 0;
            while j < LEN_SPECIFIED {
                let x = slice_index(specified, j);
                if *&raw const (*x).2 == i as u128 {
                    break 'a Some(x);
                }
                j += 1;
            }
            break 'a None;
        } {
            // result.push((found.0, found.1.clone()));
            result_t[i].write((&raw const (*found).0).read());
            result_char[i].write((&raw const (*found).1).read());
        } else if let Some(unsp) = {
            // unspecified_iter.next()
            if unspecified_iter < unspecified_len {
                let val = unspecified.as_ptr().add(unspecified_iter);
                unspecified_iter += 1;
                Some(val)
            } else {
                None
            }
        } {
            // result.push(unsp.clone());
            result_t[i].write((&raw const (*(*unsp).as_ptr()).0).read());
            result_char[i].write((&raw const (*(*unsp).as_ptr()).1).read());
        } else {
            return Err(OrderDeclsError::RanOutOfDecls);
        }
        i += 1;
    }

    if !'all_entries_from_total_exist_in_result: {
        let mut j = 0;
        while j < LEN_TOTAL {
            let ptotal = slice_index(total_unordered, j);
            let mut k = 0;
            let mut found = false;
            while k < LEN_TOTAL {
                let pcanditate = slice_index(&result_t, k).cast::<T>();
                if bitwise_partialeq(pcanditate, &raw const (*ptotal).0) {
                    if found == true {
                        panic!("duplicate entry found in ordered slice.");
                    }
                    found = true;

                }
                k += 1;
            }
            if !found {
                break 'all_entries_from_total_exist_in_result false;
            }
            j += 1;
        }
        break 'all_entries_from_total_exist_in_result true;
    } {
        return Err(OrderDeclsError::FinalSliceMissingEntries);
    }

    Ok((
        result_t.as_ptr().cast::<[T; LEN_TOTAL]>().read(),
        result_char.as_ptr().cast::<[*const c_char; LEN_TOTAL]>().read()
    ))
}

