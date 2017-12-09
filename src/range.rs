use std::str::FromStr;

use serde::de::{Deserializer, Deserialize, Error};


#[derive(Clone, Debug)]
pub struct Range {
    pub start: u32,
    pub end: u32,
}

impl Range {
    pub fn new(start: u32, end: u32) -> Range {
        return Range { start: start, end: end };
    }
    pub fn len(&self) -> u32 {
        return self.end - self.start + 1;
    }
    pub fn shift(&self, val: u32) -> Range {
        assert!(self.end - self.start + 1 >= val);
        return Range::new(self.start + val, self.end);
    }
}

impl<'a> Deserialize<'a> for Range {
    fn deserialize<D: Deserializer<'a>>(d: D) -> Result<Range, D::Error> {
        let val = String::deserialize(d)?;
        FromStr::from_str(&val[..])
        .map(|num| Range::new(num, num))
        .or_else(|_| {
            let mut pair = val.splitn(2, '-');
            Ok(Range::new(
                pair.next().and_then(|x| FromStr::from_str(x).ok())
                    .ok_or(D::Error::custom("Error parsing range"))?,
                pair.next().and_then(|x| FromStr::from_str(x).ok())
                    .ok_or(D::Error::custom("Error parsing range"))?,
            ))
        })
    }
}

pub fn in_range(ranges: &Vec<Range>, value: u32) -> bool {
    for rng in ranges.iter() {
        if rng.start <= value && rng.end >= value {
            return true;
        }
    }
    return false;
}
