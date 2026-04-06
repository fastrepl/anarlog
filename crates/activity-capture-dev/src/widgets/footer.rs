use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Paragraph, Widget},
};

use crate::app::{DetailTab, View};

pub(super) struct Footer {
    view: View,
    detail_tab: DetailTab,
}

impl Footer {
    pub(super) fn new(view: View, detail_tab: DetailTab) -> Self {
        Self { view, detail_tab }
    }
}

impl Widget for Footer {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let footer = match self.view {
            View::List => {
                "q/Esc quit  •  ↑↓ or j/k move  •  g/G home/end  •  Enter details  •  r raw JSON  •  v toggle range"
            }
            View::Details => match self.detail_tab {
                DetailTab::Details => {
                    "q quit  •  Esc/←/h back  •  ↑↓ or j/k move  •  Tab/r raw JSON  •  s save current/range"
                }
                DetailTab::Raw => {
                    "q quit  •  Esc/←/h back  •  ↑↓ or j/k move  •  Tab/d details  •  s save current/range"
                }
            },
        };

        Paragraph::new(footer).render(area, buf);
    }
}
