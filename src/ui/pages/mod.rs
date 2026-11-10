use crate::ui::*;

mod advanced;
mod color;
mod image;
mod inputs;
mod multiview;
mod overview;
mod profiles;

impl App {
    pub(crate) fn content(&mut self, ui: &mut Ui) {
        if let Conn::Err(msg) = self.conn.clone() {
            self.disconnected_banner(ui, &msg);
        }
        let section = self.section;
        let on = self.connected() || section == Section::Profiles;
        ui.add_enabled_ui(on, |ui| match section {
            Section::Overview => self.page_overview(ui),
            Section::Image => self.page_image(ui),
            Section::Color => self.page_color(ui),
            Section::Inputs => self.page_inputs(ui),
            Section::MultiView => self.page_multiview(ui),
            Section::Profiles => self.page_profiles(ui),
            Section::Advanced => self.page_advanced(ui),
        });
    }
}
