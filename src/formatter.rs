use std::fmt::{Debug, Display};

use anyhow::{Result, anyhow};

use crate::{function, types::DisFormatter};

macro_rules! spacing_longest {
    ($lines:expr, $field:ident, $spacing:expr) => {{
        let longest = $lines.iter()
            .map(|x| x.$field.len())
            .max()
            .unwrap();

        $lines.iter()
            .map(|x| {
                let field = &x.$field;
                let padding = longest - field.len() + $spacing;
                field.clone() + &" ".repeat(padding)
            })
            .collect::<Vec<_>>()
    }};
}
impl DisFormatter {
    pub fn format(&self) -> Result<String> {
        if self.lines.is_empty() {
            return Err(anyhow!("{} - cannot format with no content", function!()));
        }

        let mut lines: Vec<String>;
        if self.config.include_hex {
            lines = spacing_longest!(self.lines, hex, 2);
        } else {
            lines = vec![String::new(); self.lines.len()];
        }
        
        for (final_line, line) in lines.iter_mut().zip(&self.lines) {
            let operands = line.operands.join(&self.config.operand_separator);
            let instruction = format_dynamic(
                self.config.instruction_format.clone(),
                vec![line.opcode.clone(), operands],
            );

            *final_line += &instruction;
        }

        Ok(lines.join("\n"))
    }
}

/// Home-rolled basic version of format!() to format dynamic strings
fn format_dynamic<D>(mut f: String, args: Vec<D>) -> String
where
    D: Display + Debug,
{
    for arg in args {
        f = f.replacen("{}", &arg.to_string(), 1);
    }

    f
}
