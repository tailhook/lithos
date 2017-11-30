use quire::validate::{Sequence, Numeric, Structure};


#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub struct IdMap {
    pub inside: u32,
    pub outside: u32,
    pub count: u32,
}
pub trait IdMapExt {
    fn map_id(&self, internal_id: u32) -> Option<u32>;
}

impl IdMapExt for Vec<IdMap> {
    fn map_id(&self, internal_id: u32) -> Option<u32> {
        if self.len() == 0 {
            return Some(internal_id);
        }
        for rng in self.iter() {
            if internal_id >= rng.inside &&
                internal_id <= rng.inside + rng.count
            {
                return Some(rng.outside + (internal_id - rng.inside));
            }
        }
        None
    }
}

pub fn mapping_validator<'x>() -> Sequence<'x> {
    Sequence::new(
        Structure::new()
        .member("inside", Numeric::new())
        .member("outside", Numeric::new())
        .member("count", Numeric::new()))
}
