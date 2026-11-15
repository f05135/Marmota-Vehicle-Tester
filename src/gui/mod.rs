use tao::{
    dpi::{LogicalSize, PhysicalPosition},
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Icon, WindowBuilder},
};
use wry::{WebContext, WebViewBuilder};

pub fn launch_gui(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new();

    // Adapt initial & minimum window dimensions to the monitor's DPI and resolution
    let (init_w, init_h, min_w, min_h, pos_opt) = if let Some(mon) = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())
    {
        let scale = mon.scale_factor().max(1.0);
        let phys = mon.size();
        let avail_w = (phys.width as f64) / scale;
        let avail_h = (phys.height as f64) / scale;

        let w = (avail_w * 0.90)
            .clamp(860.0, 1440.0)
            .min(avail_w.max(640.0));
        let h = (avail_h * 0.88).clamp(600.0, 900.0).min(avail_h.max(480.0));
        let mw = (avail_w * 0.80).clamp(640.0, 1160.0).min(w);
        let mh = (avail_h * 0.78).clamp(480.0, 720.0).min(h);

        let m_pos = mon.position();
        let px = m_pos.x + (((phys.width as f64) - (w * scale)) / 2.0).max(0.0) as i32;
        let py = m_pos.y + (((phys.height as f64) - (h * scale)) / 2.0).max(0.0) as i32;
        (w, h, mw, mh, Some(PhysicalPosition::new(px, py)))
    } else {
        (1440.0, 900.0, 1160.0, 720.0, None)
    };

    let mut builder = WindowBuilder::new()
        .with_title("Marmota Vehicle Tester")
        .with_inner_size(LogicalSize::new(init_w, init_h))
        .with_min_inner_size(LogicalSize::new(min_w, min_h));

    if let Some(pos) = pos_opt {
        builder = builder.with_position(pos);
    }

    let logo_bytes = include_bytes!("../../ui/branding/logo.png");
    if let Ok(img) = image::load_from_memory(logo_bytes) {
        let rgba = img.into_rgba8();
        let (width, height) = rgba.dimensions();
        if let Ok(icon) = Icon::from_rgba(rgba.into_raw(), width, height) {
            builder = builder.with_window_icon(Some(icon));
        }
    }

    let window = builder.build(&event_loop)?;

    let mut web_context = WebContext::new(Some(crate::config::AppConfig::webview_data_dir()));
    let _webview = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_devtools(false)
        .with_url(url)
        .build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        let _keep_alive = (&window, &_webview, &web_context);
        match event {
            Event::NewEvents(StartCause::Init) => {}
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => (),
        }
    });
}
