// This is a part of lithos_ps not lithos library
use std::rc::Rc;


pub trait ColorPrinter {
    fn norm<'x>(self, &str) -> Self;
    fn red<'x>(self, val: &str) -> Self { self.norm(val) }
    fn blue<'x>(self, val: &str) -> Self { self.norm(val) }
    fn green<'x>(self, val: &str) -> Self { self.norm(val) }
}

pub struct MonotonePrinter(String);

pub trait TreePrintable {
    fn print(&self, printer: &mut ColorPrinter);
}

pub struct TreeNode {
    head: String,
    children: Vec<TreeNode>,
}


impl ColorPrinter for MonotonePrinter {
    fn norm<'x>(self, val: &str) -> MonotonePrinter {
        let MonotonePrinter(mut buf) = self;
        if buf.len() > 0 {
            buf.push(' ');
        }
        buf.push_str(val);
        return MonotonePrinter(buf);
    }
}
