use {crate::app::App, gtk4::cairo};

fn set_source_rgba_from_u8(ctx: &cairo::Context, r: u8, g: u8, b: u8, a: u8) {
    let r = (r as f64) / 255.0;
    let g = (g as f64) / 255.0;
    let b = (b as f64) / 255.0;
    let a = (a as f64) / 255.0;
    ctx.set_source_rgba(r, g, b, a);
}

fn set_source_rgb_from_u8(ctx: &cairo::Context, r: u8, g: u8, b: u8) {
    set_source_rgba_from_u8(ctx, r, g, b, 255);
}

pub fn draw(_app: &App, ctx: &cairo::Context, width: i32, height: i32) {
    // Fill in the background.
    ctx.rectangle(0.0, 0.0, width as f64, height as f64);
    set_source_rgb_from_u8(ctx, 63, 63, 255);
    ctx.fill();

    // TODO
}
