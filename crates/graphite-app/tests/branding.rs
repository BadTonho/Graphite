#[path = "../build_support/icons.rs"]
mod icons;
const SOURCE: &[u8] = include_bytes!("../../../assets/branding/graphite.png");
#[test]
fn artwork_produces_all_native_sizes_and_preserves_window_pixels() {
    let generated = icons::generate(SOURCE).unwrap();
    assert_eq!(
        generated.rgba.len(),
        (icons::WINDOW_SIZE * icons::WINDOW_SIZE * 4) as usize
    );
    let expected = image::load_from_memory(SOURCE)
        .unwrap()
        .resize(256, 256, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    assert_eq!(generated.rgba, expected.as_raw().as_slice());
    assert_eq!(&generated.ico[..4], &[0, 0, 1, 0]);
    assert_eq!(
        u16::from_le_bytes(generated.ico[4..6].try_into().unwrap()) as usize,
        icons::ICON_SIZES.len()
    );
    for (i, &size) in icons::ICON_SIZES.iter().enumerate() {
        let entry = &generated.ico[6 + i * 16..6 + (i + 1) * 16];
        let count = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        let frame = image::load_from_memory(&generated.ico[offset..offset + count]).unwrap();
        assert_eq!((frame.width(), frame.height()), (size, size));
    }
    let embedded = include_bytes!(concat!(env!("OUT_DIR"), "/graphite-icon.rgba"));
    assert_eq!(embedded.as_slice(), generated.rgba);
}
#[test]
fn malformed_artwork_fails_instead_of_silently_removing_the_icon() {
    assert!(icons::generate(b"not a PNG").is_err());
}
#[test]
fn nonsquare_artwork_is_centered_with_transparent_padding() {
    let source = image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 0, 0, 255]));
    let mut png = std::io::Cursor::new(Vec::new());
    source.write_to(&mut png, image::ImageFormat::Png).unwrap();
    let generated = icons::generate(png.get_ref()).unwrap();
    let canvas = image::RgbaImage::from_raw(256, 256, generated.rgba).unwrap();
    assert_eq!(canvas.get_pixel(128, 0).0, [0, 0, 0, 0]);
    assert_eq!(canvas.get_pixel(128, 128).0, [255, 0, 0, 255]);
    assert_eq!(canvas.get_pixel(128, 255).0, [0, 0, 0, 0]);
}
#[test]
fn linux_launcher_uses_the_same_identity_as_the_window() {
    let launcher = include_str!("../../../packaging/linux/graphite.desktop");
    for setting in [
        "Name=Graphite",
        "Exec=graphite %F",
        "Icon=graphite",
        "StartupWMClass=graphite",
    ] {
        assert!(launcher.lines().any(|line| line == setting));
    }
}
#[cfg(windows)]
#[test]
fn windows_executable_contains_the_generated_icon_group() {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt};
    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryExW(path: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        fn FindResourceW(module: *mut c_void, name: *const u16, kind: *const u16) -> *mut c_void;
        fn SizeofResource(module: *mut c_void, resource: *mut c_void) -> u32;
        fn FreeLibrary(module: *mut c_void) -> i32;
    }
    let path: Vec<_> = std::ffi::OsStr::new(env!("CARGO_BIN_EXE_graphite"))
        .encode_wide()
        .chain(Some(0))
        .collect();
    // Load as data only: the test never starts an editor window or executes its code.
    let module = unsafe { LoadLibraryExW(path.as_ptr(), std::ptr::null_mut(), 2) };
    assert!(!module.is_null(), "{}", std::io::Error::last_os_error());
    let resource = unsafe { FindResourceW(module, 1usize as *const u16, 14usize as *const u16) };
    let size = if resource.is_null() {
        0
    } else {
        unsafe { SizeofResource(module, resource) }
    };
    unsafe {
        FreeLibrary(module);
    }
    assert_eq!(
        size as usize,
        6 + icons::ICON_SIZES.len() * 14,
        "Executable must embed every icon size"
    );
}
