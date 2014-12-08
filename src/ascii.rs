// This is a part of lithos_ps not lithos library
use std::io::IoError;
use std::io::Writer;


pub trait Printer {
    fn norm(self, &str) -> Self;
    fn red(self, val: &str) -> Self { self.norm(val) }
    fn blue(self, val: &str) -> Self { self.norm(val) }
    fn green(self, val: &str) -> Self { self.norm(val) }
    fn unwrap(self) -> String;
}

pub struct MonotonePrinter(String);
pub struct ColorPrinter(String);

pub struct TreeNode {
    head: String,
    children: Vec<TreeNode>,
}


impl Printer for MonotonePrinter {
    fn norm<'x>(self, val: &str) -> MonotonePrinter {
        let MonotonePrinter(mut buf) = self;
        if buf.len() > 0 {
            buf.push(' ');
        }
        buf.push_str(val);
        return MonotonePrinter(buf);
    }
    fn unwrap(self) -> String {
        let MonotonePrinter(buf) = self;
        return buf;
    }
}

impl Printer for ColorPrinter {
    fn norm(self, val: &str) -> ColorPrinter {
        let ColorPrinter(mut buf) = self;
        if buf.len() > 0 {
            buf.push(' ');
        }
        buf.push_str(val);
        return ColorPrinter(buf);
    }
    fn red(self, val: &str) -> ColorPrinter {
        let ColorPrinter(mut buf) = self;
        if buf.len() > 0 {
            buf.push(' ');
        }
        buf.push_str("\x33[31m\x33[1m");
        buf.push_str(val);
        buf.push_str("\x33[0m\x3322m");
        return ColorPrinter(buf);
    }
    fn blue(self, val: &str) -> ColorPrinter {
        let ColorPrinter(mut buf) = self;
        if buf.len() > 0 {
            buf.push(' ');
        }
        buf.push_str("\x33[34m\x33[1m");
        buf.push_str(val);
        buf.push_str("\x33[0m\x3322m");
        return ColorPrinter(buf);
    }
    fn green(self, val: &str) -> ColorPrinter {
        let ColorPrinter(mut buf) = self;
        if buf.len() > 0 {
            buf.push(' ');
        }
        buf.push_str("\x33[32m\x33[1m");
        buf.push_str(val);
        buf.push_str("\x33[0m\x3322m");
        return ColorPrinter(buf);
    }
    fn unwrap(self) -> String {
        let ColorPrinter(buf) = self;
        return buf;
    }
}

impl TreeNode {
    pub fn print<T:Writer>(&self, writer: &mut T) -> Result<(), IoError> {
        try!(writer.write_str(self.head.as_slice()));
        try!(writer.write_char('\n'))
        self._print_children(writer, "  ")
    }
    pub fn _print_children<T:Writer>(&self, writer: &mut T, indent: &str)
        -> Result<(), IoError>
    {
        if self.children.len() >= 2 {
            let childindent = indent.to_string() + "│   ";
            for child in self.children[..self.children.len()-1].iter() {
                try!(writer.write_str(indent));
                try!(writer.write_str("├─"));
                try!(writer.write_str(child.head.as_slice()));
                try!(writer.write_char('\n'))
                try!(child._print_children(writer, childindent.as_slice()));
            }
        }
        if let Some(child) = self.children.last() {
            let childindent = indent.to_string() + "    ";
            try!(writer.write_str(indent));
            try!(writer.write_str("└─"));
            try!(writer.write_str(child.head.as_slice()));
            try!(writer.write_char('\n'))
            try!(child._print_children(writer, childindent.as_slice()));
        }
        return Ok(());
    }

}

#[cfg(test)]
mod test {
    use super::TreeNode;
    use std::io::MemWriter;

    fn write_tree(node: &TreeNode) -> String {
        let mut buf = MemWriter::new();
        node.print(&mut buf).unwrap();
        return String::from_utf8(buf.unwrap()).unwrap();
    }

    #[test]
    fn test_one_node() {
        assert_eq!(write_tree(&TreeNode {
            head: "parent".to_string(),
            children: vec!()
        }).as_slice(), "\
parent\n\
        ");
    }

    #[test]
    fn test_many_nodes() {
        assert_eq!(write_tree(&TreeNode {
            head: "parent".to_string(),
            children: vec!(TreeNode {
                head: "child1".to_string(),
                children: vec!(TreeNode {
                    head: "subchild".to_string(),
                    children: vec!(),
                }),
            }, TreeNode {
                head: "child2".to_string(),
                children: vec!(TreeNode {
                    head: "subchild".to_string(),
                    children: vec!(),
                }),
            })
        }).as_slice(), "\
parent
  ├─child1
  │   └─subchild
  └─child2
      └─subchild\n\
        ");
    }
}
