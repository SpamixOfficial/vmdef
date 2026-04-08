// https://stackoverflow.com/a/40234666
#[macro_export]
macro_rules! function {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        name.strip_suffix("::f").unwrap()
    }};
}

#[macro_export]
macro_rules! option_to_res {
    ($f:expr, $($x:expr),*) => {
        $f.ok_or_else(|| anyhow!("{} - {}", function!(), format!($($x),*)))
    };
}

#[macro_export]
macro_rules! unwrap_or {
    ($f:expr, $($x:expr),*) => {
        $f.unwrap_or(Err(anyhow!("{} - {}", function!(), format!($($x),*))))
    };
}

#[macro_export]
macro_rules! buf_as_usize {
    ($b:expr) => {{
        let mut as_usize = [0u8; size_of::<usize>()];
        as_usize[..min($b.len(), size_of::<usize>())]
            .copy_from_slice(&$b[..min($b.len(), size_of::<usize>())]);

        usize::from_le_bytes(as_usize)
    }};
    ($b:expr, $l:expr) => {{
        let mut as_usize = [0u8; size_of::<usize>()];
        as_usize[..min($l, size_of::<usize>())].copy_from_slice(&$b[..min($l, size_of::<usize>())]);

        usize::from_le_bytes(as_usize)
    }};
}

#[macro_export]
macro_rules! log_if_verbose {
    ($v:expr, $($x:expr),*) => {{
        if $v {
            eprintln!($($x),*);
        }
    }};
}
