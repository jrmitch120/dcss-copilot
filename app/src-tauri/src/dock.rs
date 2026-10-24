//! Where the panel docks beside the browser window holding the game, and where it collapses to.
//!
//! Everything here is in logical pixels, the units the browser reports its windows in. A desktop
//! that mixes scale factors across monitors may place the panel a little off: the browser's and
//! the desktop's ideas of where one monitor ends and the next begins can differ there.

use copilot_webtiles::relay_protocol::WindowBounds;
use serde::{Deserialize, Serialize};

/// Below this width the browser isn't shrunk to make room: webtiles needs about this much to
/// draw the map with the stats beside it.
const MIN_BROWSER_WIDTH: f64 = 800.0;

/// The Collapsed Pill's size: room for the threat label, one line and the badge.
const PILL_WIDTH: f64 = 360.0;
/// See [`PILL_WIDTH`].
const PILL_HEIGHT: f64 = 44.0;

/// How far the Collapsed Pill sits in from the browser's corner.
const PILL_MARGIN: f64 = 16.0;

/// A rectangle on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Its left edge.
    pub left: f64,
    /// Its top edge.
    pub top: f64,
    /// Its width.
    pub width: f64,
    /// Its height.
    pub height: f64,
}

impl Rect {
    fn right(&self) -> f64 {
        self.left + self.width
    }

    fn bottom(&self) -> f64 {
        self.top + self.height
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        (self.left..self.right()).contains(&x) && (self.top..self.bottom()).contains(&y)
    }

    /// The area `self` and `other` share, if any.
    fn intersection(&self, other: &Self) -> Option<Self> {
        let left = self.left.max(other.left);
        let top = self.top.max(other.top);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (right > left && bottom > top).then_some(Self {
            left,
            top,
            width: right - left,
            height: bottom - top,
        })
    }

    fn area(&self) -> f64 {
        self.width * self.height
    }
}

impl From<WindowBounds> for Rect {
    fn from(bounds: WindowBounds) -> Self {
        Self {
            left: f64::from(bounds.left),
            top: f64::from(bounds.top),
            width: f64::from(bounds.width),
            height: f64::from(bounds.height),
        }
    }
}

/// The edge of the browser window the panel docks to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DockSide {
    /// Its left edge.
    Left,
    /// Its right edge.
    #[default]
    Right,
}

/// Where the docked panel goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Docked {
    /// The panel's outer bounds.
    pub panel: Rect,
    /// It covers the browser's edge, there being no room beside it, so it must stay on top.
    pub over_browser: bool,
    /// The bounds to shrink the browser to so the panel fits beside it, when Make Room is on and
    /// the browser needs shrinking.
    pub make_room: Option<WindowBounds>,
}

/// The work area of the screen `browser` is mostly on, out of `screens`: the one holding its
/// centre, else the one it overlaps most.
#[must_use]
pub fn screen_of(browser: Rect, screens: &[Rect]) -> Option<Rect> {
    let (x, y) = (
        browser.left + browser.width / 2.0,
        browser.top + browser.height / 2.0,
    );
    screens
        .iter()
        .find(|screen| screen.contains(x, y))
        .or_else(|| {
            screens.iter().max_by(|a, b| {
                let overlap =
                    |screen: &Rect| browser.intersection(screen).map_or(0.0, |r| r.area());
                overlap(a).total_cmp(&overlap(b))
            })
        })
        .copied()
}

/// Docks a panel `panel_width` wide to the `side` of `browser`, on the screen whose work area is
/// `screen`: beside the browser where there is room, else, with `make_room`, beside a browser
/// shrunk to make it, else over the browser's edge.
#[must_use]
pub fn dock(
    browser: Rect,
    screen: Rect,
    side: DockSide,
    panel_width: f64,
    make_room: bool,
) -> Docked {
    let top = browser.top.max(screen.top);
    let height = browser.bottom().min(screen.bottom()) - top;
    let at = |left: f64| Rect {
        left,
        top,
        width: panel_width,
        height,
    };
    let (room, shrunk) = match side {
        DockSide::Right => (
            screen.right() - browser.right(),
            Rect {
                width: screen.right() - panel_width - browser.left,
                ..browser
            },
        ),
        DockSide::Left => (
            browser.left - screen.left,
            Rect {
                left: screen.left + panel_width,
                width: browser.right() - screen.left - panel_width,
                ..browser
            },
        ),
    };
    let beside = |browser: Rect| match side {
        DockSide::Right => at(browser.right()),
        DockSide::Left => at(browser.left - panel_width),
    };
    if room >= panel_width {
        return Docked {
            panel: beside(browser),
            over_browser: false,
            make_room: None,
        };
    }
    if make_room && shrunk.width >= MIN_BROWSER_WIDTH {
        return Docked {
            panel: beside(shrunk),
            over_browser: false,
            make_room: Some(bounds(shrunk)),
        };
    }
    let visible = browser.intersection(&screen).unwrap_or(browser);
    Docked {
        panel: match side {
            DockSide::Right => at(visible.right() - panel_width),
            DockSide::Left => at(visible.left),
        },
        over_browser: true,
        make_room: None,
    }
}

/// Where the Collapsed Pill goes: in the bottom corner of `browser` on the `side` the panel
/// docks to, within the screen whose work area is `screen`.
#[must_use]
pub fn pill(browser: Rect, screen: Rect, side: DockSide) -> Rect {
    let visible = browser.intersection(&screen).unwrap_or(screen);
    Rect {
        left: match side {
            DockSide::Right => visible.right() - PILL_WIDTH - PILL_MARGIN,
            DockSide::Left => visible.left + PILL_MARGIN,
        },
        top: visible.bottom() - PILL_HEIGHT - PILL_MARGIN,
        width: PILL_WIDTH,
        height: PILL_HEIGHT,
    }
}

/// `rect` as window bounds, rounded to whole pixels.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Screen sizes fit easily.
fn bounds(rect: Rect) -> WindowBounds {
    WindowBounds {
        left: rect.left.round() as i32,
        top: rect.top.round() as i32,
        width: rect.width.round().max(0.0) as u32,
        height: rect.height.round().max(0.0) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PANEL: f64 = 420.0;

    fn rect(left: f64, top: f64, width: f64, height: f64) -> Rect {
        Rect {
            left,
            top,
            width,
            height,
        }
    }

    /// A 1920 × 1080 screen with a 25-pixel menu bar along the top.
    const SCREEN: Rect = Rect {
        left: 0.0,
        top: 25.0,
        width: 1920.0,
        height: 1055.0,
    };

    #[test]
    fn the_panel_docks_beside_the_browser_where_there_is_room() {
        let browser = rect(100.0, 60.0, 1200.0, 900.0);

        assert_eq!(
            dock(browser, SCREEN, DockSide::Right, PANEL, false),
            Docked {
                panel: rect(1300.0, 60.0, PANEL, 900.0),
                over_browser: false,
                make_room: None,
            }
        );
        let browser = rect(600.0, 60.0, 1200.0, 900.0);
        assert_eq!(
            dock(browser, SCREEN, DockSide::Left, PANEL, true).panel,
            rect(180.0, 60.0, PANEL, 900.0)
        );
    }

    #[test]
    fn without_room_beside_a_maximized_browser_the_panel_covers_its_edge() {
        assert_eq!(
            dock(SCREEN, SCREEN, DockSide::Right, PANEL, false),
            Docked {
                panel: rect(1500.0, 25.0, PANEL, 1055.0),
                over_browser: true,
                make_room: None,
            }
        );
        assert_eq!(
            dock(SCREEN, SCREEN, DockSide::Left, PANEL, false).panel,
            rect(0.0, 25.0, PANEL, 1055.0)
        );
    }

    #[test]
    fn make_room_shrinks_the_browser_so_the_panel_fits_beside_it() {
        let right = dock(SCREEN, SCREEN, DockSide::Right, PANEL, true);
        assert_eq!(
            right,
            Docked {
                panel: rect(1500.0, 25.0, PANEL, 1055.0),
                over_browser: false,
                make_room: Some(WindowBounds {
                    left: 0,
                    top: 25,
                    width: 1500,
                    height: 1055
                }),
            }
        );

        let left = dock(SCREEN, SCREEN, DockSide::Left, PANEL, true);
        assert_eq!(left.panel, rect(0.0, 25.0, PANEL, 1055.0));
        assert_eq!(
            left.make_room,
            Some(WindowBounds {
                left: 420,
                top: 25,
                width: 1500,
                height: 1055
            })
        );
    }

    #[test]
    fn make_room_never_shrinks_the_browser_too_narrow_for_webtiles() {
        let narrow_screen = rect(0.0, 25.0, 1100.0, 800.0);

        let docked = dock(narrow_screen, narrow_screen, DockSide::Right, PANEL, true);

        assert!(docked.over_browser);
        assert_eq!(docked.make_room, None);
    }

    #[test]
    fn the_panel_stays_on_the_screen_of_a_browser_hanging_off_it() {
        let browser = rect(1000.0, 0.0, 1200.0, 1200.0);

        assert_eq!(
            dock(browser, SCREEN, DockSide::Right, PANEL, false).panel,
            rect(1500.0, 25.0, PANEL, 1055.0)
        );
    }

    #[test]
    fn the_collapsed_pill_sits_in_the_browsers_bottom_corner_on_the_docked_side() {
        let browser = rect(100.0, 60.0, 1200.0, 900.0);

        assert_eq!(
            pill(browser, SCREEN, DockSide::Right),
            rect(924.0, 900.0, PILL_WIDTH, PILL_HEIGHT)
        );
        assert_eq!(
            pill(browser, SCREEN, DockSide::Left),
            rect(116.0, 900.0, PILL_WIDTH, PILL_HEIGHT)
        );
    }

    #[test]
    fn the_browser_is_on_the_screen_holding_its_centre_or_else_the_one_it_overlaps_most() {
        let left_screen = rect(-1440.0, 0.0, 1440.0, 900.0);
        let screens = [SCREEN, left_screen];

        assert_eq!(
            screen_of(rect(-1300.0, 40.0, 1000.0, 800.0), &screens),
            Some(left_screen)
        );
        assert_eq!(
            screen_of(rect(-200.0, 40.0, 1000.0, 800.0), &screens),
            Some(SCREEN)
        );
        // Its centre is in the gap below the shorter screen.
        assert_eq!(
            screen_of(rect(-1000.0, 600.0, 1100.0, 800.0), &screens),
            Some(left_screen)
        );
        assert_eq!(screen_of(SCREEN, &[]), None);
    }
}
