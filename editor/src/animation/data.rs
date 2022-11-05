use fyrox::{
    asset::{ResourceData, ResourceState},
    core::visitor::prelude::*,
    resource::animation::AnimationResource,
};
use std::path::PathBuf;

pub struct DataModel {
    pub saved: bool,
    pub resource: AnimationResource,
}

impl DataModel {
    pub fn save(&mut self, path: PathBuf) {
        if !self.saved {
            self.resource.data_ref().set_path(path.clone());
            if let ResourceState::Ok(ref mut state) = *self.resource.state() {
                let mut visitor = Visitor::new();
                state
                    .animation_definition
                    .visit("Definition", &mut visitor)
                    .unwrap();
                visitor.save_binary(&path).unwrap();
            }
            self.saved = true;
        }
    }
}
