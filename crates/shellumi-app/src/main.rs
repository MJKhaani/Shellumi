use gpui::{
    div, prelude::*, px, rgb, size, App, Application, Bounds, Context, Window, WindowBounds,
    WindowOptions,
};

struct Shellumi;

impl Render for Shellumi {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .gap_3()
            .bg(rgb(0x111827))
            .text_color(rgb(0xf3f4f6))
            .child(div().text_2xl().child("Shellumi"))
            .child("The desktop foundation is under development.")
            .child("SSH and terminal sessions are not yet available in this window.")
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Shellumi),
        )
        .expect("failed to open Shellumi window");
        cx.activate(true);
    });
}
