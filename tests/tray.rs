use tray_icon::{Icon, TrayIconBuilder};

#[test]
fn create_more_tray() {
    let _main_tray = TrayIconBuilder::new()
        .with_icon(create_solid_icon(255, 0, 0, 255).unwrap())
        .build()
        .unwrap();

    let _secondary_tray = TrayIconBuilder::new()
        .with_icon(create_solid_icon(0, 255, 0, 255).unwrap())
        .build()
        .unwrap();

    let _secondary_tray_2 = TrayIconBuilder::new()
        .with_icon(create_solid_icon(0, 0, 255, 255).unwrap())
        .build()
        .unwrap();

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn create_solid_icon(r: u8, g: u8, b: u8, a: u8) -> Option<Icon> {
    let pixel_count = (20 * 20) as usize;
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    for _ in 0..pixel_count {
        rgba.push(r);
        rgba.push(g);
        rgba.push(b);
        rgba.push(a);
    }
    Icon::from_rgba(rgba, 20, 20).ok()
}
