use crate::find;
use crate::input::InputMode;
use tui::widgets::Widget;
use tui::widgets::{StatefulWidget, Block, Borders};
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::text::{Span, Spans};
use tui::style::{Style, Modifier, Color};
use tui::symbols::line;

#[derive(Debug)]
pub struct CsvTable<'a> {
    header: Vec<String>,
    rows: &'a [Vec<String>],
}

impl<'a> CsvTable<'a> {
    pub fn new(header: &[String], rows: &'a [Vec<String>]) -> Self {
        let _header = header.to_vec();
        Self {
            header: _header,
            rows,
        }
    }
}

impl<'a> CsvTable<'a> {

    fn get_column_widths(&self) -> Vec<u16> {
        let mut column_widths = Vec::new();
        for s in self.header.iter() {
            column_widths.push(s.len() as u16);
        }
        for row in self.rows.iter() {
            for (i, value) in row.iter().enumerate() {
                let v = column_widths.get_mut(i).unwrap();
                let value_len = value.len() as u16;
                if *v < value_len {
                    *v = value_len;
                }
            }
        }
        for w in column_widths.iter_mut() {
            *w += 4;
        }
        column_widths
    }

    fn render_row_numbers(
        &self,
        buf: &mut Buffer,
        state: &mut CsvTableState,
        area: Rect,
        num_rows: usize,
    ) -> u16 {

        // TODO: better to derminte width from total number of records, so this is always fixed
        let max_row_num = state.rows_offset as usize + num_rows + 1;
        let mut section_width = format!("{}", max_row_num).len() as u16;

        // Render line numbers
        let y_first_record = area.y;
        let mut y = area.y;
        for i in 0..num_rows {
            let row_num = i + state.rows_offset as usize + 1;
            let row_num_formatted = format!("{}", row_num);
            let style = Style::default()
                .fg(Color::Rgb(64, 64, 64));
            let span = Span::styled(row_num_formatted, style);
            buf.set_span(0, y, &span, section_width);
            y += 1;
            if y >= area.bottom() {
                break;
            }
        }
        section_width = section_width + 2 + 1;  // one char reserved for line; add one for symmetry

        state.borders_state = Some(
            BordersState {
                x_row_separator: section_width,
                y_first_record,
            }
        );

        // Add more space before starting first column
        section_width += 2;

        section_width
    }

    fn render_header_borders(&self, buf: &mut Buffer, area: Rect) -> (u16, u16) {
        let block = Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Rgb(64, 64, 64)));
        let height = 3;
        let area = Rect::new(0, 0, area.width, height);
        block.render(area, buf);
        // y pos of header text and next line
        (height.saturating_sub(2), height)
    }

    fn render_other_borders(&self, buf: &mut Buffer, area: Rect, state: &CsvTableState) {
        // TODO: probably should move all these lines rendering somewhere else
        // Render vertical separator
        if state.borders_state.is_none() {
            return;
        }

        let borders_state = state.borders_state.as_ref().unwrap();
        let y_first_record = borders_state.y_first_record;
        let section_width = borders_state.x_row_separator;

        let line_number_block = Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(Color::Rgb(64, 64, 64)));
        let line_number_area = Rect::new(
            0,
            y_first_record,
            section_width,
            area.height,
        );
        line_number_block.render(line_number_area, buf);

        // Intersection with header separator
        buf.get_mut(section_width - 1, y_first_record - 1)
            .set_symbol(line::HORIZONTAL_DOWN);

        // Status separator at the bottom (rendered here first for the interesection)
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Rgb(64, 64, 64)));
        let status_separator_area = Rect::new(
            0,
            y_first_record + area.height,
            area.width,
            1,
        );
        block.render(status_separator_area, buf);

        // Intersection with bottom separator
        buf.get_mut(section_width - 1, y_first_record + area.height)
            .set_symbol(line::HORIZONTAL_UP);
    }

    fn render_row(
        &self,
        buf: &mut Buffer,
        state: &mut CsvTableState,
        column_widths: &[u16],
        area: Rect,
        x: u16,
        y: u16,
        is_header: bool,
        row: &[String],
        row_index: Option<usize>,
    ) {
        let mut x_offset_header = x;
        let mut remaining_width = area.width.saturating_sub(x);
        let cols_offset = state.cols_offset as usize;
        let mut has_more_cols_to_show = false;
        let mut num_cols_rendered = 0;
        for (col_index, (hname, &hlen)) in row.iter().zip(column_widths).enumerate() {
            if col_index < cols_offset {
                continue;
            }
            if remaining_width < hlen {
                has_more_cols_to_show = true;
                break;
            }
            let mut style = Style::default();
            if is_header {
                style = style.add_modifier(Modifier::BOLD);

            }
            match &state.finder_state {
                FinderState::FinderActive(active) if (*hname).contains(active.target.as_str()) => {
                    let mut highlight_style = Style::default().fg(Color::Rgb(200, 0, 0));
                    if let Some(hl) = &active.found_record {
                        if let Some(row_index) = row_index {
                            // TODO: vec::contains slow or does it even matter?
                            if row_index == hl.row_index() && hl.column_indices().contains(&col_index) {
                                highlight_style = highlight_style.bg(Color::LightYellow);
                            }
                        }
                    }
                    let p_span = Span::styled(active.target.as_str(), highlight_style);
                    let splitted = (*hname).split(active.target.as_str());
                    let mut spans = vec![];
                    for part in splitted {
                        let span = Span::styled(part, style);
                        spans.push(span);
                        spans.push(p_span.clone());
                    }
                    spans.pop();
                    let spans = Spans::from(spans);
                    buf.set_spans(x_offset_header, y, &spans, hlen);
                }
                _ => {
                    let span = Span::styled((*hname).as_str(), style);
                    buf.set_span(x_offset_header, y, &span, hlen);
                }
            };
            x_offset_header += hlen;
            remaining_width = remaining_width.saturating_sub(hlen);
            num_cols_rendered += 1;
        }
        state.set_num_cols_rendered(num_cols_rendered);
        state.set_more_cols_to_show(has_more_cols_to_show);
    }

    fn render_status(&self, area: Rect, buf: &mut Buffer, state: &mut CsvTableState) {

        // Content of status line (separator already plotted elsewhere)
        let style = Style::default().fg(Color::Rgb(128, 128, 128));
        let mut content: String;
        if let BufferState::Enabled(buffer_mode, buf) = &state.buffer_content {
            content = buf.to_owned();
            match buffer_mode {
                InputMode::GotoLine => {
                    content = format!("Go to line: {}", content);
                }
                InputMode::Find => {
                    content = format!("Find: {}", content);
                }
                _ => {}
            }
        }
        else {
            content = state.filename.to_string();

            let total_str = if state.total_line_number.is_some() {
                format!("{}", state.total_line_number.unwrap())
            }  else {
                "?".to_owned()
            };
            content += format!(
                " [Row {}/{}, Col {}/{}]",
                state.rows_offset + 1,
                total_str,
                state.cols_offset + 1,
                state.total_cols,
            ).as_str();

            if let FinderState::FinderActive(s) = &state.finder_state {
                content += format!(" {}", s.status_line()).as_str();
            }

            if let Some(elapsed) = state.elapsed {
                content += format!(" [{}ms]", elapsed).as_str();
            }

            if !state.debug.is_empty() {
                content += format!(" (debug: {})", state.debug).as_str();
            }
        }
        let span = Span::styled(content, style);
        buf.set_span(area.x, area.bottom().saturating_sub(1), &span, area.width);
    }
}

impl<'a> StatefulWidget for CsvTable<'a> {
    type State = CsvTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {

        // TODO: draw relative to the provided area

        if area.area() == 0 {
            return;
        }

        let status_height = 2;
        let column_widths = self.get_column_widths();
        let (y_header, y_first_record) = self.render_header_borders(buf, area);

        // row area: including row numbers and row content
        let rows_area = Rect::new(
            area.x,
            y_first_record,
            area.width,
            area.height.saturating_sub(y_first_record).saturating_sub(status_height),
        );

        let row_num_section_width = self.render_row_numbers(
            buf,
            state,
            rows_area,
            self.rows.len(),
        );

        self.render_row(
            buf,
            state,
            &column_widths,
            rows_area,
            row_num_section_width,
            y_header,
            true,
            &self.header,
            None,
        );

        let mut y_offset = y_first_record;
        for (rel_row_index, row) in self.rows.iter().enumerate() {
            let row_index = rel_row_index.saturating_add(state.rows_offset as usize);
            self.render_row(
                buf,
                state,
                &column_widths,
                rows_area,
                row_num_section_width,
                y_offset,
                false,
                row,
                Some(row_index),
            );
            y_offset += 1;
            if y_offset >= rows_area.bottom() {
                break;
            }
        }

        let status_area = Rect::new(
            area.x,
            area.bottom().saturating_sub(status_height),
            area.width,
            status_height,
        );
        self.render_status(status_area, buf, state);

        self.render_other_borders(buf, rows_area, state);
    }
}

pub enum BufferState {
    Disabled,
    Enabled(InputMode, String),
}

pub enum FinderState {
    FinderInactive,
    FinderActive(FinderActiveState),
}

impl FinderState {
    pub fn from_finder(finder: &find::Finder) -> FinderState {
        let active_state = FinderActiveState::new(finder);
        FinderState::FinderActive(active_state)
    }
}

pub struct FinderActiveState {
    find_complete: bool,
    total_found: u64,
    cursor_index: Option<u64>,
    target: String,
    found_record: Option<find::FoundRecord>,
}

impl FinderActiveState {

    pub fn new(finder: &find::Finder) -> Self {
        FinderActiveState {
            find_complete: finder.done(),
            total_found: finder.count() as u64,
            cursor_index: finder.cursor().map(|x| x as u64),
            target: finder.target(),
            found_record: finder.current(),
        }
    }

    fn status_line(&self) -> String {
        let plus_marker;
        let line;
        if self.total_found == 0 {
            if self.find_complete {
                line = "Not found".to_owned();
            }
            else {
                line = "Finding...".to_owned();
            }
        }
        else {
            if self.find_complete {
                plus_marker = "";
            }
            else {
                plus_marker = "+";
            }
            let cursor_str = if self.cursor_index.is_none() {
                "-".to_owned()
            } else {
                (self.cursor_index.unwrap() + 1).to_string()
            };
            line = format!(
                "{}/{}{}",
                cursor_str,
                self.total_found,
                plus_marker,
            );
        }
        format!("[\"{}\": {}]", self.target, line)
    }
}

struct BordersState {
    x_row_separator: u16,
    y_first_record: u16,
}

pub struct CsvTableState {
    // TODO: types appropriate?
    pub rows_offset: u64,
    pub cols_offset: u64,
    pub num_cols_rendered: u64,
    pub more_cols_to_show: bool,
    filename: String,
    total_line_number: Option<usize>,
    total_cols: usize,
    pub elapsed: Option<f64>,
    buffer_content: BufferState,
    pub finder_state: FinderState,
    borders_state: Option<BordersState>,
    debug: String,
}

impl CsvTableState {

    pub fn new(filename: String, total_cols: usize) -> Self {
        Self {
            rows_offset: 0,
            cols_offset: 0,
            num_cols_rendered: 0,
            more_cols_to_show: true,
            filename,
            total_line_number: None,
            total_cols,
            elapsed: None,
            buffer_content: BufferState::Disabled,
            finder_state: FinderState::FinderInactive,
            borders_state: None,
            debug: "".into(),
        }
    }

    pub fn set_rows_offset(&mut self, offset: u64) {
        self.rows_offset = offset;
    }

    pub fn set_cols_offset(&mut self, offset: u64) {
        self.cols_offset = offset;
    }

    fn set_more_cols_to_show(&mut self, value: bool) {
        self.more_cols_to_show = value;
    }

    pub fn has_more_cols_to_show(&mut self) -> bool {
        self.more_cols_to_show
    }

    fn set_num_cols_rendered(&mut self, n: u64) {
        self.num_cols_rendered = n;
    }

    pub fn set_total_line_number(&mut self, n: usize) {
        self.total_line_number = Some(n);
    }

    pub fn set_buffer(&mut self, mode: InputMode, buf: &str) {
        self.buffer_content = BufferState::Enabled(mode, buf.to_string());
    }

    pub fn reset_buffer(&mut self) {
        self.buffer_content = BufferState::Disabled;
    }

}