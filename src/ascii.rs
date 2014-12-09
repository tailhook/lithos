// This is a part of lithos_ps not lithos library
use std::io::IoError;
use std::io::Writer;
use std::cmp::max;
use std::fmt::Show;


pub trait Printer {
    fn norm(self, &Show) -> Self;
    fn red(self, val: &Show) -> Self { self.norm(val) }
    fn blue(self, val: &Show) -> Self { self.norm(val) }
    fn green(self, val: &Show) -> Self { self.norm(val) }
    fn unwrap(self) -> String;
}

pub struct MonotonePrinter(pub String);
pub struct ColorPrinter(pub String);

pub struct TreeNode {
    pub head: String,
    pub children: Vec<TreeNode>,
}

pub enum Column {
    Text(Vec<String>),
    Bytes(Vec<uint>),
    Ordinal(Vec<uint>),
    Percent(Vec<f32>),
}


impl Printer for MonotonePrinter {
    fn norm<'x>(self, val: &Show) -> MonotonePrinter {
        let MonotonePrinter(mut buf) = self;
        if buf.len() > 0 {
            buf.push(' ');
        }
        buf.push_str(val.to_string().as_slice());
        return MonotonePrinter(buf);
    }
    fn unwrap(self) -> String {
        let MonotonePrinter(buf) = self;
        return buf;
    }
}

impl Printer for ColorPrinter {
    fn norm(self, val: &Show) -> ColorPrinter {
        let ColorPrinter(mut buf) = self;
        if buf.len() > 0 {
            buf.push(' ');
        }
        buf.push_str(val.to_string().as_slice());
        return ColorPrinter(buf);
    }
    fn red(self, val: &Show) -> ColorPrinter {
        let ColorPrinter(mut buf) = self;
        if buf.len() > 0 {
            buf.push(' ');
        }
        buf.push_str("\x1b[31m\x1b[1m");
        buf.push_str(val.to_string().as_slice());
        buf.push_str("\x1b[0m\x1b[22m");
        return ColorPrinter(buf);
    }
    fn blue(self, val: &Show) -> ColorPrinter {
        let ColorPrinter(mut buf) = self;
        if buf.len() > 0 {
            buf.push(' ');
        }
        buf.push_str("\x1b[34m\x1b[1m");
        buf.push_str(val.to_string().as_slice());
        buf.push_str("\x1b[0m\x1b[22m");
        return ColorPrinter(buf);
    }
    fn green(self, val: &Show) -> ColorPrinter {
        let ColorPrinter(mut buf) = self;
        if buf.len() > 0 {
            buf.push(' ');
        }
        buf.push_str("\x1b[32m\x1b[1m");
        buf.push_str(val.to_string().as_slice());
        buf.push_str("\x1b[0m\x1b[22m");
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

pub fn render_table(columns: &[(&'static str, Column)]) {
    let mut out_cols = Vec::new();
    for &(ref title, ref col) in columns.iter() {
        match *col {
            Bytes(ref items) => {
                let max = items.iter().max().map(|&x| x).unwrap_or(1);
                let (k, unit) = match max {
                    1 ... 10240 => (1f64, "B"),
                    10241 ... 10485760 => (1024f64, "kiB"),
                    10485761 ... 10737418240 => (1048576f64, "MiB"),
                    _ => (1073741824f64, "GiB"),
                };
                let mut values = vec!(format!("{1:>0$}", 7+unit.len(), title));
                values.extend(items.iter().map(
                    |x| format!("{:7.1f}{}", (*x as f64) / k, unit)));
                values.reverse();
                out_cols.push(values);
            }
            Text(ref items) => {
                let maxlen = max(3,
                    items.iter().map(|x| x.len()).max().unwrap_or(3));
                let mut values = vec!(format!("{1:<0$}", maxlen, title));
                values.extend(items.iter().map(
                    |x| format!("{1:<0$}", maxlen, *x)));
                values.reverse();
                out_cols.push(values);
            }
            Ordinal(ref items) => {
                let maxlen = max(3, items.iter().map(
                    |x| format!("{}", x).len()).max().unwrap_or(3));
                let mut values = vec!(format!("{1:>0$}", maxlen, title));
                values.extend(items.iter().map(
                    |x| format!("{1:0$}", maxlen, *x)));
                values.reverse();
                out_cols.push(values);
            }
            Percent(ref items) => {
                let mut values = vec!(format!("{:>5}", title));
                values.extend(items.iter().map(
                    |x| format!("{:>5.1f}", *x)));
                values.reverse();
                out_cols.push(values);
            }
        }
    }
    loop {
        for ref mut lst in out_cols.iter_mut() {
            if lst.len() == 0 {
                return;
            }
            print!("{} ", lst.pop().unwrap());
        }
        println!("");
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
