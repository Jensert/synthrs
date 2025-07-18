use ratatui::widgets::Widget;

pub struct Slider {
    content: String,
    value: f32,
}
impl Widget for Slider {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        area.left();
    }
}
