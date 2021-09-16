use std::fmt;

pub struct Table<const N: usize> {
    headers: [String; N],
    rows: Vec<[String; N]>,
    widths: [usize; N],
    alignments: [Alignment; N],
}

impl<const N: usize> Table<N> {
    pub fn new(headers: [impl Into<String>; N]) -> Self {
        let headers = headers.map(Into::into);
        let mut widths = [0; N];
        for (i, width) in widths.iter_mut().enumerate() {
            *width = headers[i].len();
        }
        Table {
            headers,
            rows: vec![],
            widths,
            alignments: [Alignment::Left; N],
        }
    }

    pub fn align(&mut self, alignments: [Alignment; N]) -> &mut Self {
        self.alignments = alignments;
        self
    }

    pub fn row(&mut self, row: [impl Into<String>; N]) -> &mut Self {
        let row = row.map(Into::into);
        for (i, width) in self.widths.iter_mut().enumerate() {
            *width = (*width).max(row[i].len());
        }
        self.rows.push(row);
        self
    }

    #[inline(always)]
    fn fmt_row(
        &self,
        f: &mut fmt::Formatter<'_>,
        row: &[String; N],
    ) -> Result<(), std::fmt::Error> {
        for (i, column) in row.iter().enumerate() {
            match self.alignments[i] {
                Alignment::Left => write!(f, "{: <width$}  ", column, width = self.widths[i])?,
                Alignment::Center => write!(f, "{: ^width$}  ", column, width = self.widths[i])?,
                Alignment::Right => write!(f, "{: >width$}  ", column, width = self.widths[i])?,
            }
        }
        writeln!(f)?;
        Ok(())
    }
}

impl<const N: usize> fmt::Display for Table<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.fmt_row(f, &self.headers)?;
        for i in 0..self.headers.len() {
            write!(f, "{:-<width$}  ", "", width = self.widths[i])?;
        }
        writeln!(f)?;
        for row in &self.rows {
            self.fmt_row(f, row)?;
        }
        for i in 0..self.headers.len() {
            write!(f, "{:-<width$}  ", "", width = self.widths[i])?;
        }
        writeln!(f)?;
        self.fmt_row(f, &self.headers)?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub enum Alignment {
    Left,
    Center,
    Right,
}
