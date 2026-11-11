use crate::ui::*;

mod advanced;
mod color;
mod image;
mod inputs;
mod multiview;
mod overview;
mod profiles;
mod rescue;

impl App {
    pub(crate) fn content(&mut self, ui: &mut Ui) {
        if let Conn::Err(msg) = self.conn.clone() {
            self.disconnected_banner(ui, &msg);
        }
        let section = self.section;
        // The rescue page must stay usable without a picture or a monitor:
        // that is the whole point of arming a restore before reconnecting.
        let on = self.connected() || matches!(section, Section::Profiles | Section::Rescue);
        ui.add_enabled_ui(on, |ui| match section {
            Section::Overview => self.page_overview(ui),
            Section::Image => self.page_image(ui),
            Section::Color => self.page_color(ui),
            Section::Inputs => self.page_inputs(ui),
            Section::MultiView => self.page_multiview(ui),
            Section::Profiles => self.page_profiles(ui),
            Section::Rescue => self.page_rescue(ui),
            Section::Advanced => self.page_advanced(ui),
        });
    }
}
