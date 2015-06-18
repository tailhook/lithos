use std::str::{FromStr, CharIndices};
use std::borrow::Borrow;


pub trait NextValue {
    fn next_value<T:FromStr>(&mut self) -> Result<T, ()>;
    fn nth_value<T:FromStr>(&mut self, i: usize) -> Result<T, ()>;
}

impl<I, T: Borrow<str>> NextValue for I
    where I: Iterator<Item=T>
{

    fn next_value<A:FromStr>(&mut self) -> Result<A, ()> {
        self.next().ok_or(())
        .and_then(|x| FromStr::from_str(x.borrow()).map_err(|_| ()))
    }

    fn nth_value<A:FromStr>(&mut self, i: usize) -> Result<A, ()> {
        self.nth(i).ok_or(())
        .and_then(|x| FromStr::from_str(x.borrow()).map_err(|_| ()))
    }

}

pub trait NextStr<'a> {
    fn next_str(&mut self) -> Result<&'a str, ()>;
    fn nth_str(&mut self, i: usize) -> Result<&'a str, ()>;
}

impl<'a, I> NextStr<'a> for I
    where I: Iterator<Item=&'a str>
{
    fn next_str(&mut self) -> Result<&'a str, ()> {
        return self.next().ok_or(());
    }
    fn nth_str(&mut self, i: usize) -> Result<&'a str, ()> {
        return self.nth(i).ok_or(());
    }
}

pub struct Words<'a> {
    src: &'a str,
    iter: CharIndices<'a>,
}

impl<'a> Words<'a> {
    fn skip_ws(&mut self) -> Option<(usize, char)> {
        loop {
            if let Some((idx, ch)) = self.iter.next() {
                if !ch.is_whitespace() {
                    return Some((idx, ch));
                }
            } else {
                return None
            }
        }
    }
}

impl<'a> Iterator for Words<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        if let Some((start_idx, _)) = self.skip_ws() {
            loop {
                if let Some((idx, ch)) = self.iter.next() {
                    if ch.is_whitespace() {
                        return Some(&self.src[start_idx..idx]);
                    }
                } else {
                    return Some(&self.src[start_idx..]);
                }
            }
        } else {
            return None;
        }
    }
}

pub fn words<'a, 'b: 'a, B: Borrow<str> + ?Sized + 'a>(src: &'b B) -> Words<'a> {
    return Words {
        src: src.borrow(),
        iter: src.borrow().char_indices(),
        };
}
