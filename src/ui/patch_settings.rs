//! **Item Patcher Settings**: the mod's row in the game's Options dialog, on
//! the Competition tab ([`super::options_dialog`] finds the dialog), and the
//! window its button opens.
//!
//! The window is the item page of another mod's balance editor (toxicsnek's
//! Auto Balance Patcher), drawn over again for this mod's own patches
//! ([`crate::patches`]): a list of the items a patch can touch down the left,
//! with a search box over it, and for the item picked a card a tier, the
//! legendary and its radiant, with a row for every number a patch can move.
//! A row has
//!
//! - **Current**: what the number reads in this save, patches and all;
//! - **New Value**: a box to type another into. It is not taken at once: it
//!   waits for the next manual item patch, which is then made of these
//!   numbers and of nothing else (`patches::consider`), and says so in its
//!   notes. Emptied, the number waits no longer;
//! - **Minimum** and **Maximum**: the least and the most an automatic patch
//!   may make the number, in place of the band every number has
//!   (`patches::tuning`), which the empty boxes show;
//! - **Protected**: on, no automatic patch moves the number.
//!
//! All of it is the save's (`patches::state`): another save has its own.
//!
//! The footer has the rest. On its left a toggle, **Changed Items Only**,
//! narrows the list to the items with a New Value waiting, and beside it a
//! line says how many are. On its right, Back and then **Queue Manual
//! Patch**, which queues the patch that releases them
//! (`patches::queue_hotfix`). That button is greyed out, and takes no click,
//! while there is nothing to release and from when a patch is queued until
//! it has gone out; the line on the left says which.
//!
//! Over the page, beside the cross, **Import** and **Export** carry all of
//! it from one save to another, through a file the player picks in a Windows
//! file dialog that opens in the mod's folder ([`transfer`]). What they did
//! is said on the footer's line.
//!
//! # Where the window hangs
//!
//! Under the layout root the dialog is in, the pause menu's, as its last
//! child: at equal `z` the later of two siblings is drawn over the earlier,
//! and that is the only thing that puts a panel over the dialog (see
//! `strategy_ui` on `z`). It is spawned anew each time it is opened, so that
//! it is the last child whenever game code has built its dialog again, and it
//! goes with the pause menu when that closes.
//!
//! # The text boxes
//!
//! A box is read on every frame while the window is up: the only event a
//! text box has is the one for a finished edit, and a number typed and left
//! in its box has to count. What a box held when it was last read is kept
//! ([`TextBox`]), and a reading that differs is the player's typing. A box
//! the window fills itself is not believed to differ until it has once read
//! what was written into it, since a write is not known to show on the next
//! frame.
//!
//! # Clicks
//!
//! A handler only notes which node was clicked ([`CLICKS`]); what the click
//! means is worked out on the next frame, here, where the window's state is.
//! A node is registered each time it is spawned, since a registration in the
//! pause menu's layout does not outlast its node, and a click that arrives
//! twice for that reason is noted once.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

use mod_api_stable::{StableClient, UiEventKindV1};

use super::options_dialog;
use crate::patches::base::{self, Member};
use crate::patches::state::Limit;
use crate::patches::{balance, fields, text, tuning};

mod transfer;

// The row's node in the Options dialog, and its button under it.
const ROW: &str = "riot_patcher_settings";
const BUTTON_UNDER: &str = ".open";

// The window's node, under the layout root the dialog is in.
const WINDOW: &str = "riot_item_patcher";

// The row's strings: this mod's key in the game's text, and the English.
const ROW_TITLE: (&str, &str) = ("item_patch.settings_row", "Item Patcher Settings");
const ROW_OPEN: (&str, &str) = ("item_patch.settings_open", "Open");
const ROW_HINT: (&str, &str) = (
    "item_patch.settings_hint",
    "Set item numbers by hand and release them as a manual item patch, and choose what the automatic item patches may change.",
);

// Frames after a node is spawned or comes into view in which an event on it
// is not taken for a click. A node was seen to be sent an event with no
// click behind it; what sent it is not known, and the moment a node is
// built, shown or painted is when the host would answer with one.
const SETTLE_FRAMES: u32 = 12;

// Frames a box the window wrote into is waited on to read what was written.
const WRITE_WAIT: u8 = 30;

// The most a number may be set to by hand, either side of zero.
const LARGEST: f64 = 1_000_000.0;

// The sheet the item icons are in.
const ITEM_SHEET: &str = crate::strategy_ui::ICON_SHEET;

// The arrow between a number and the one waiting to replace it, in the font
// the game's own patch notes take it from.
const ARROW: &str = "<f#asset/base/font/set/symbol>\u{2192}<f>";

// The colours of a row's note: a number that waits, and one that cannot be
// taken. And of the footer's line once a patch is queued.
const WAITING_COLOR: &str = "#f0b35aff";
const ERROR_COLOR: &str = "#ff6b6bff";
const QUEUED_COLOR: &str = "#5fd38dff";

// A pill, on and off: its fill, and the word on it. The Protected pills and
// the footer's toggle.
const PILL_ON: (&str, &str) = ("#37d5b3ff", "#073f36ff");
const PILL_OFF: (&str, &str) = ("#3a3d4aff", "#c2c6ceff");

// The looks of the footer's queue button, as [`queue_look`] numbers them:
// a patch can be queued, one is queued, or there is nothing for one to
// release. Never 0, which is a button not painted yet.
const QUEUE_READY: u8 = 1;
const QUEUE_QUEUED: u8 = 2;
const QUEUE_EMPTY: u8 = 3;

// One entry of the item list is this tall, and this far from the next.
const ENTRY_HEIGHT: u32 = 44;
const ENTRY_GAP: u32 = 2;

// A card: what is over its rows (its name and the column headings), one
// row, what is under the last, and the gap to the next card.
const CARD_HEAD: u32 = 88;
const FIELD_HEIGHT: u32 = 46;
const CARD_FOOT: u32 = 10;
const CARD_GAP: u32 = 16;

// Frames [`sync`] has run.
static FRAME: AtomicU32 = AtomicU32::new(0);

// The row: whether the dialog was up on the last frame, whether the row was
// on screen, and the frame its button was last disturbed on.
static ROW_UP: AtomicBool = AtomicBool::new(false);
static ROW_SHOWN: AtomicBool = AtomicBool::new(false);
static ROW_SETTLED_AT: AtomicU32 = AtomicU32::new(0);

// The row's button has been clicked: the window is to be opened.
static OPEN: AtomicBool = AtomicBool::new(false);

// The nodes of the window clicked since the last frame, by path.
static CLICKS: Mutex<Vec<String>> = Mutex::new(Vec::new());

// The window while it is open.
static OPENED: Mutex<Option<Window>> = Mutex::new(None);

// The item the window last showed, to open on again.
static LAST_ITEM: Mutex<Option<String>> = Mutex::new(None);

// -- layouts --------------------------------------------------------------------

// The layouts are written as they would be in a `.ui` file, with `%NAME%`
// where something is filled in ([`fill`]).

// The row, shaped like the layout's own: a title as wide as theirs, and one
// button over the 852 pixels their option cells have.
const ROW_SOURCE: &str = r##"%NODE%:empty {
  width: 1160px;
  height: 40px;

  #header:label {
    @"asset/base/style/main#label";
    width: 305px;
    height: 40px;
    align_y: Center;
    size: 16;
    text: "%TITLE%";
  }

  #open:color_icon_button {
    @"asset/base/style/main#secondary_button";
    x: 255px;
    width: 852px;
    height: 40px;

    text: {
      text: "%OPEN%";
      font: "asset/base/font/set/bold";
      size: 16;
      align_x: Center;
      align_y: Center;
    }

    hint: "%HINT%";
  }
}
"##;

// The window: a backdrop that takes every click meant for what is under it,
// and the panel, with the item list on its left, the page of the item picked
// on its right and a footer.
const WINDOW_SOURCE: &str = r##"%NODE%:empty {
  width: 1920px;
  height: 1080px;
  z: 500;

  anchor_x: 0.5;
  pivot_x: 0.5;
  anchor_y: 0.5;
  pivot_y: 0.5;

  #backdrop:color_icon_button {
    width: 100%;
    height: 100%;

    btn: {
      color: #00000090;
    }
  }

  #popup:color {
    width: 1860px;
    height: 1040px;

    anchor_x: 0.5;
    pivot_x: 0.5;
    anchor_y: 0.5;
    pivot_y: 0.5;

    color: #161721ff;

    rounding: Uniform {
      rounding: 12;
    }

    #header:label {
      @"asset/base/style/main#bold_label";
      x: 24px;
      y: 20px;
      width: 900px;
      height: 36px;
      size: 24;
      align_y: Center;
      text: "%TITLE%";
    }

    #close:button {
      width: 18px;
      height: 18px;

      anchor_x: 1;
      pivot_x: 1;

      x: -24px;
      y: 29px;

      source: "asset/base/ui/icons/cross";
      color: #c2c6ceff;

      hover: {
        color: #e8e8e8ff;
      }

      active: {
        color: #e8e8e8ff;
      }
    }

    #import:color_icon_button {
      @"asset/base/style/main#secondary_button";
      anchor_x: 1;
      pivot_x: 1;
      x: -238px;
      y: 18px;
      width: 160px;
      height: 40px;

      text: {
        font: "asset/base/font/set/bold";
        text: "%IMPORT%";
        align_x: Center;
        align_y: Center;
        size: 15;
      }

      hint: "%IMPORT_HINT%";
    }

    #export:color_icon_button {
      @"asset/base/style/main#secondary_button";
      anchor_x: 1;
      pivot_x: 1;
      x: -66px;
      y: 18px;
      width: 160px;
      height: 40px;

      text: {
        font: "asset/base/font/set/bold";
        text: "%EXPORT%";
        align_x: Center;
        align_y: Center;
        size: 15;
      }

      hint: "%EXPORT_HINT%";
    }

    #sidebar:empty {
      x: 24px;
      y: 76px;
      width: 360px;
      height: 884px;

      #search:text_edit {
        @"asset/base/style/main#text_edit";
        width: 360px;
        height: 42px;
        size: 15;
        align_y: Center;

        padding: {
          left: 44px;
          top: 5px;
          right: 15px;
          bottom: 5px;
        }

        placeholder: "%SEARCH%";
        max_length: 60;
      }

      #search_icon:image {
        source: "asset/base/ui/banpick/fi-rr-search";
        x: 14px;
        y: 11px;
        width: 20px;
        height: 20px;
        color: #858d9dff;
        ignore_event: true;
      }

      #items:scroll_view {
        y: 54px;
        width: 360px;
        height: 830px;

        speed: 100;

        bar_width: 4;
        bar: {
          source: "asset/base/sprite/white";
          color: #60ddc2ff;

          hover: {
            color: #ecfbf8ff;
          }
        }

        back: {
          source: "asset/base/sprite/white";
          color: #4a4c56ff;
        }

        #rows:empty {
          width: 350px;
          height: 10px;
        }
      }
    }

    #vline:image {
      source: "asset/base/sprite/white";
      x: 408px;
      y: 76px;
      width: 1px;
      height: 884px;
      color: #2c2f3cff;
      ignore_event: true;
    }

    #main:empty {
      x: 432px;
      y: 72px;
      width: 1404px;
      height: 892px;

      #title:label {
        @"asset/base/style/main#bold_label";
        width: 1404px;
        height: 38px;
        size: 27;
        align_y: Center;
        text: "";
      }

      #subtitle:label {
        @"asset/base/style/main#label";
        y: 38px;
        width: 1404px;
        height: 36px;
        size: 15;
        line_height: 18;
        color: #a5a5abff;
        align_y: Center;
        text: "%SUBTITLE%";
      }

      #page:scroll_view {
        y: 76px;
        width: 1404px;
        height: 816px;

        speed: 100;

        bar_width: 4;
        bar: {
          source: "asset/base/sprite/white";
          color: #60ddc2ff;

          hover: {
            color: #ecfbf8ff;
          }
        }

        back: {
          source: "asset/base/sprite/white";
          color: #4a4c56ff;
        }

        #rows:empty {
          width: 1394px;
          height: 10px;
        }
      }
    }

    #footer:color {
      anchor_y: 1;
      pivot_y: 1;
      width: 1860px;
      height: 64px;

      color: #0f1016ff;

      rounding: Individual {
        bottom_left: 12;
        bottom_right: 12;
      }

      #only_fill:color {
        x: 24px;
        y: 10px;
        width: 250px;
        height: 44px;
        color: #3a3d4aff;
        rounding: Uniform { rounding: 9; }
      }

      #only:color_selectable {
        x: 24px;
        y: 10px;
        width: 250px;
        height: 44px;

        hover_sound: "asset/base/sound/sfx/UI_mouse_hover";
        click_sound: "asset/base/sound/sfx/UI_mouse_click";

        image: {
          color: #00000000;
          rounding: Uniform { rounding: 9; }
          hover: { color: #ffffff22; }
        }

        selected_image: {
          color: #00000000;
          rounding: Uniform { rounding: 9; }
          hover: { color: #ffffff22; }
        }

        text: "";
      }

      #only_text:label {
        @"asset/base/style/main#bold_label";
        x: 24px;
        y: 10px;
        width: 250px;
        height: 44px;
        size: 15;
        color: #c2c6ceff;
        align_x: Center;
        align_y: Center;
        ignore_event: true;
        text: "%ONLY%";
      }

      #status_icon:image {
        visible: false;
        source: "asset/base/ui/icons/check";
        x: 290px;
        y: 21px;
        width: 22px;
        height: 22px;
        color: #5fd38dff;
        ignore_event: true;
      }

      #status:label {
        @"asset/base/style/main#bold_label";
        x: 290px;
        width: 1000px;
        height: 64px;
        size: 15;
        line_height: 20;
        color: #a5a5abff;
        align_y: Center;
        text: "";
      }

      #back:color_icon_button {
        @"asset/base/style/main#secondary_button";
        anchor_x: 1;
        pivot_x: 1;
        x: -296px;
        y: 10px;
        width: 220px;
        height: 44px;

        text: {
          font: "asset/base/font/set/bold";
          text: "%BACK%";
          align_x: Center;
          align_y: Center;
          size: 16;
        }
      }

      #queue_off:color {
        anchor_x: 1;
        pivot_x: 1;
        x: -24px;
        y: 10px;
        width: 260px;
        height: 44px;
        color: #20342da6;
        rounding: Uniform { rounding: 8; }

        #text:label {
          @"asset/base/style/main#bold_label";
          width: 260px;
          height: 44px;
          size: 16;
          color: #a7b8b3a6;
          align_x: Center;
          align_y: Center;
          text: "%QUEUE%";
        }
      }

      #queue:color_icon_button {
        @"asset/base/style/main#primary_button";
        visible: false;
        anchor_x: 1;
        pivot_x: 1;
        x: -24px;
        y: 10px;
        width: 260px;
        height: 44px;

        text: {
          font: "asset/base/font/set/bold";
          text: "%QUEUE%";
          align_x: Center;
          align_y: Center;
          size: 16;
        }
      }
    }
  }
}
"##;

// One item of the list. The selectable looks the same picked or not: which
// item is picked is `#hl`'s to show, since a selectable picks itself on a
// click and is not told when another is picked.
const ENTRY_SOURCE: &str = r##"i%N%:empty {
  y: %Y%px;
  width: 350px;
  height: 44px;

  #hl:color {
    visible: false;
    width: 350px;
    height: 44px;
    color: #37d5b3ff;
    stroke: 1;
    back_color: #1a2a2cff;
    ignore_event: true;
    rounding: Uniform { rounding: 8; }
  }

  #sel:color_selectable {
    width: 350px;
    height: 44px;

    hover_sound: "asset/base/sound/sfx/UI_mouse_hover";
    click_sound: "asset/base/sound/sfx/UI_mouse_click";

    image: {
      color: #00000000;
      back_color: #00000000;
      stroke: 1;
      rounding: Uniform { rounding: 8; }
      hover: { color: #4a4c56ff; }
    }

    selected_image: {
      color: #00000000;
      back_color: #00000000;
      stroke: 1;
      rounding: Uniform { rounding: 8; }
      hover: { color: #4a4c56ff; }
    }

    text: "";
  }

  #icon:image {
    source: "%SHEET%";
    rect_tag: "%FRAME%";
    x: 8px;
    y: 6px;
    width: 32px;
    height: 32px;
    ignore_event: true;
  }

  #name:label {
    @"asset/base/style/main#label";
    x: 54px;
    width: 290px;
    height: 44px;
    size: 15;
    align_y: Center;
    ignore_event: true;
    text: "%NAME%";
  }
}
"##;

// One tier's card: its name over the column headings, and its rows.
const CARD_SOURCE: &str = r##"c%N%:color {
  y: %Y%px;
  width: 1394px;
  height: %HEIGHT%px;

  color: #2c2f3cff;
  stroke: 1;
  back_color: #1b1d27ff;

  rounding: Uniform {
    rounding: 10;
  }

  #icon:image {
    source: "%SHEET%";
    rect_tag: "%FRAME%";
    x: 20px;
    y: 12px;
    width: 36px;
    height: 36px;
    ignore_event: true;
  }

  #title:label {
    @"asset/base/style/main#bold_label";
    x: 68px;
    y: 12px;
    width: 900px;
    height: 36px;
    size: 19;
    align_y: Center;
    text: "%NAME%";
  }

  #h_setting:label {
    @"asset/base/style/main#bold_label";
    x: 20px;
    y: 56px;
    width: 320px;
    height: 28px;
    size: 13;
    color: #a5a5abff;
    align_y: Center;
    text: "%H_SETTING%";
  }

  #h_current:label {
    @"asset/base/style/main#bold_label";
    x: 360px;
    y: 56px;
    width: 150px;
    height: 28px;
    size: 13;
    color: #a5a5abff;
    align_y: Center;
    text: "%H_CURRENT%";
  }

  #h_value:label {
    @"asset/base/style/main#bold_label";
    x: 520px;
    y: 56px;
    width: 150px;
    height: 28px;
    size: 13;
    color: #a5a5abff;
    align_y: Center;
    text: "%H_VALUE%";
  }

  #h_min:label {
    @"asset/base/style/main#bold_label";
    x: 680px;
    y: 56px;
    width: 150px;
    height: 28px;
    size: 13;
    color: #a5a5abff;
    align_y: Center;
    text: "%H_MIN%";
  }

  #h_max:label {
    @"asset/base/style/main#bold_label";
    x: 840px;
    y: 56px;
    width: 150px;
    height: 28px;
    size: 13;
    color: #a5a5abff;
    align_y: Center;
    text: "%H_MAX%";
  }

  #h_protected:label {
    @"asset/base/style/main#bold_label";
    x: 1010px;
    y: 56px;
    width: 200px;
    height: 28px;
    size: 13;
    color: #a5a5abff;
    align_y: Center;
    text: "%H_PROTECTED%";
  }

%ROWS%
}
"##;

// One number of a tier: what it is called, what it reads, the three boxes,
// the Protected pill and a note. The pill is three nodes, as in the editor
// this is drawn after: a fill, a selectable over it that takes the click and
// looks the same in both its states, and the word.
const FIELD_SOURCE: &str = r##"  #f%N%:empty {
    y: %Y%px;
    width: 1394px;
    height: 46px;

    #line:image {
      source: "asset/base/sprite/white";
      x: 16px;
      width: 1362px;
      height: 1px;
      color: #262938ff;
      ignore_event: true;
    }

    #name:label {
      @"asset/base/style/main#label";
      x: 20px;
      y: 4px;
      width: 330px;
      height: 22px;
      size: 15;
      align_y: Center;
      text: "%LABEL%";
    }

    #sub:label {
      @"asset/base/style/main#label";
      x: 20px;
      y: 26px;
      width: 330px;
      height: 16px;
      size: 11;
      color: #7c7f8aff;
      align_y: Center;
      text: "%SUB%";
    }

    #current:color {
      x: 360px;
      y: 7px;
      width: 140px;
      height: 32px;
      color: #2c2f3cff;
      stroke: 1;
      back_color: #13141cff;
      rounding: Uniform { rounding: 6; }

      #text:label {
        @"asset/base/style/main#label";
        x: 12px;
        width: 120px;
        height: 32px;
        size: 15;
        align_y: Center;
        text: "%CURRENT%";
      }
    }

    #value:text_edit {
      @"asset/base/style/main#text_edit";
      x: 520px;
      y: 7px;
      width: 140px;
      height: 32px;
      size: 15;
      align_y: Center;

      padding: {
        left: 12px;
        top: 4px;
        right: 12px;
        bottom: 4px;
      }

      max_length: 12;
    }

    #min:text_edit {
      @"asset/base/style/main#text_edit";
      x: 680px;
      y: 7px;
      width: 140px;
      height: 32px;
      size: 15;
      align_y: Center;

      padding: {
        left: 12px;
        top: 4px;
        right: 12px;
        bottom: 4px;
      }

      placeholder: "%LOW%";
      max_length: 12;
    }

    #max:text_edit {
      @"asset/base/style/main#text_edit";
      x: 840px;
      y: 7px;
      width: 140px;
      height: 32px;
      size: 15;
      align_y: Center;

      padding: {
        left: 12px;
        top: 4px;
        right: 12px;
        bottom: 4px;
      }

      placeholder: "%HIGH%";
      max_length: 12;
    }

    #pill:color {
      x: 1010px;
      y: 10px;
      width: 56px;
      height: 26px;
      color: #3a3d4aff;
      rounding: Uniform { rounding: 13; }
    }

    #auto:color_selectable {
      x: 1010px;
      y: 10px;
      width: 56px;
      height: 26px;

      hover_sound: "asset/base/sound/sfx/UI_mouse_hover";
      click_sound: "asset/base/sound/sfx/UI_mouse_click";

      image: {
        color: #00000000;
        rounding: Uniform { rounding: 13; }
        hover: { color: #ffffff22; }
      }

      selected_image: {
        color: #00000000;
        rounding: Uniform { rounding: 13; }
        hover: { color: #ffffff22; }
      }

      text: "";
    }

    #auto_text:label {
      @"asset/base/style/main#bold_label";
      x: 1010px;
      y: 10px;
      width: 56px;
      height: 26px;
      size: 13;
      color: #c2c6ceff;
      align_x: Center;
      align_y: Center;
      ignore_event: true;
      text: "%OFF%";
    }

    #note:label {
      @"asset/base/style/main#label";
      x: 1090px;
      width: 290px;
      height: 46px;
      size: 13;
      color: #a5a5abff;
      align_y: Center;
      text: "";
    }
  }
"##;

// `template` with each `%NAME%` of `values` filled in.
fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for &(name, value) in values {
        out = out.replace(name, value);
    }
    out
}

// `text` as it can stand in a layout's string: less what would end the
// string, and less what a label would read as markup.
fn clean(text: &str) -> String {
    options_dialog::escape(text)
        .chars()
        .filter(|c| !matches!(c, '<' | '>' | '%'))
        .collect()
}

// One of the window's strings in the game's language, fit for a layout.
fn said(key: &str, english: &str) -> String {
    clean(&text::ui_string(key, english))
}

// One of them that has a number in it, `{N}`, with `count` for the number.
fn said_of(key: &str, english: &str, count: &str) -> String {
    clean(&text::ui_string(key, english).replace("{N}", count))
}

// -- the window's state ---------------------------------------------------------

// A text box, and what it held when it was last read.
struct TextBox {
    path: String,
    shown: String,
    // The box has read what `shown` says: from then on a reading that
    // differs is the player's typing.
    confirmed: bool,
    // Frames left to wait for that.
    wait: u8,
}

impl TextBox {
    // The box at `path`, with `text` written into it where there is any.
    fn filled(ctx: &mut StableClient<'_>, path: String, text: String) -> Self {
        let written = !text.is_empty();
        if written {
            ctx.ui_set_text_edit_text(&path, &text);
        }
        Self {
            path,
            shown: text,
            confirmed: !written,
            wait: WRITE_WAIT,
        }
    }

    // What the player has typed into the box since it was last read, when
    // they have typed anything.
    fn typed(&mut self, ctx: &StableClient<'_>) -> Option<String> {
        let read = ctx.ui_text_edit_text(&self.path)?;
        if read == self.shown {
            self.confirmed = true;
            return None;
        }
        if !self.confirmed {
            // What was written into it does not show yet. Given up on after
            // a while: the box then stands as it reads, and is not taken to
            // have been typed into.
            self.wait = self.wait.saturating_sub(1);
            if self.wait == 0 {
                self.confirmed = true;
                self.shown = read;
            }
            return None;
        }
        self.shown = read.clone();
        Some(read)
    }
}

// What a box holds, read as a number.
enum Typed {
    Empty,
    Number(f64),
    Unreadable,
}

fn typed(text: &str) -> Typed {
    let text = text.trim().trim_end_matches('%').trim().replace(',', ".");
    if text.is_empty() {
        return Typed::Empty;
    }
    match text.parse::<f64>() {
        Ok(number) if number.is_finite() => Typed::Number(number),
        _ => Typed::Unreadable,
    }
}

// One number of one tier, as its row has it.
struct Row {
    // The row's node.
    path: String,
    // The tier, and the number's config field.
    key: String,
    field: String,
    // The number before any patch, and as the save has it now.
    unpatched: f64,
    current: f64,
    // It is kept as a whole number.
    whole: bool,
    // The game keeps it unsigned: it cannot be taken under what the item
    // registered with (`fields::Rule::unsigned`).
    unsigned: bool,
    // It reads as a percentage.
    percent: bool,
    // The number waiting for the next manual patch, and what the player
    // allows an automatic one.
    staged: Option<f64>,
    limit: Limit,
    value: TextBox,
    min: TextBox,
    max: TextBox,
    // Why the New Value box cannot be taken, and why the other two cannot.
    value_error: Option<String>,
    limit_error: Option<String>,
}

// One item of the list: an item in every tier it comes in.
struct Entry {
    // The item's family, as `patches::base` keeps it.
    id: String,
    // Its tiers, by key: what a number waiting for a patch is kept under.
    keys: Vec<String>,
    name: String,
    // The name as the search box is matched against it.
    folded: String,
    frame: String,
}

struct Window {
    // The window's node.
    root: String,
    entries: Vec<Entry>,
    // What the search box held when it was last read.
    filter: String,
    // The footer's toggle is on: the list has only the items with a number
    // waiting for a patch.
    only: bool,
    // What the footer's queue button was last painted as ([`queue_look`]),
    // 0 for not yet, and the frame it was.
    queue_look: u8,
    queue_at: u32,
    // The item whose page is up, by its place among `entries`.
    picked: Option<usize>,
    // The cards of that page, and their rows.
    cards: usize,
    rows: Vec<Row>,
    // The frames the window and the page were spawned on.
    opened_at: u32,
    paged_at: u32,
    // What the footer says.
    status: String,
    // What an import or an export did, which the footer says in place of
    // how the manual patch stands until the player changes something.
    notice: Option<String>,
}

impl Window {
    fn under(&self, rest: &str) -> String {
        format!("{}{rest}", self.root)
    }

    fn list(&self) -> String {
        self.under(".popup.sidebar.items.rows")
    }

    fn page(&self) -> String {
        self.under(".popup.main.page.rows")
    }
}

// -- the numbers ----------------------------------------------------------------

// A number as a box or a label shows it.
fn number(value: f64) -> String {
    // Four places: a number set by hand is typed, and one a patch made is
    // on its steps.
    text::number_text((value * 10_000.0).round() / 10_000.0)
}

// The fields of `member` in the order their rows come: the stats first, as
// a tooltip lists them, and the passives' numbers after.
fn ordered(member: &Member) -> Vec<&String> {
    let mut ordered: Vec<&String> = member.values.keys().collect();
    ordered.sort_by_key(|field| {
        fields::FLAT
            .iter()
            .position(|flat| *flat == field.as_str())
            .unwrap_or(fields::FLAT.len())
    });
    ordered
}

// The band an automatic patch keeps a number of `unpatched` in where the
// player gave it no least or most: its low end and its high end.
fn band(unpatched: f64, whole: bool) -> (f64, f64) {
    let tuning = tuning::tuning();
    let (a, b) = (unpatched * tuning.min_ratio, unpatched * tuning.max_ratio);
    let (low, high) = (a.min(b), a.max(b));
    if whole {
        (low.ceil(), high.floor())
    } else {
        (low, high)
    }
}

// Whether the player may set a number of `unpatched` to `value` by hand, or
// why not: one kept `whole`, or `unsigned` (see [`Row`]).
fn settable(whole: bool, unsigned: bool, unpatched: f64, value: f64) -> Result<(), String> {
    if whole && value.fract() != 0.0 {
        return Err(said("settings_err_whole", "Whole numbers only"));
    }
    // On the side of zero it is on, and not zero: an item that had the
    // number would have it no longer, which no tooltip can say.
    if value == 0.0 || (value > 0.0) != (unpatched > 0.0) || value.abs() > LARGEST {
        return Err(said("settings_err_sign", "Must be above 0"));
    }
    if unsigned && value < unpatched {
        return Err(said_of(
            "settings_err_floor",
            "Cannot go below {N}",
            &number(unpatched),
        ));
    }
    Ok(())
}

// Takes what the player typed into `row`'s New Value box.
fn take_value(row: &mut Row, typed_text: &str) {
    let (staged, error) = match typed(typed_text) {
        Typed::Empty => (None, None),
        Typed::Unreadable => (None, Some(said("settings_err_number", "Not a number"))),
        Typed::Number(value) => match settable(row.whole, row.unsigned, row.unpatched, value) {
            // What the number reads already is nothing to release.
            Ok(()) => (((value - row.current).abs() > 1e-9).then_some(value), None),
            Err(error) => (None, Some(error)),
        },
    };
    row.staged = staged;
    row.value_error = error;
    crate::patches::edit_settings(|state| state.stage(&row.key, &row.field, staged));
}

// Takes what the Minimum and Maximum boxes of `row` hold.
fn take_limits(row: &mut Row) {
    let bound = |held: &TextBox| match typed(&held.shown) {
        Typed::Empty => Ok(None),
        Typed::Number(value) => Ok(Some(value)),
        Typed::Unreadable => Err(()),
    };
    let (min, max) = (bound(&row.min), bound(&row.max));
    row.limit_error = match (min, max) {
        (Err(()), _) | (_, Err(())) => Some(said("settings_err_number", "Not a number")),
        (Ok(Some(min)), Ok(Some(max))) if min > max => {
            Some(said("settings_err_order", "Minimum is above maximum"))
        }
        _ => None,
    };
    row.limit.min = min.ok().flatten();
    row.limit.max = max.ok().flatten();
    keep_limit(row);
}

fn keep_limit(row: &Row) {
    let limit = row.limit;
    crate::patches::edit_settings(|state| state.set_limit(&row.key, &row.field, limit));
}

// -- drawing --------------------------------------------------------------------

// A number of `row` with its percent sign.
fn shown(row: &Row, value: f64) -> String {
    format!("{}{}", number(value), if row.percent { "%" } else { "" })
}

// Writes `row`'s note: why a box cannot be taken, or the number that waits.
fn write_note(ctx: &mut StableClient<'_>, row: &Row) {
    let note = match (&row.value_error, &row.limit_error, row.staged) {
        (Some(error), _, _) | (None, Some(error), _) => format!("<{ERROR_COLOR}>{error}<>"),
        (None, None, Some(staged)) => format!(
            "<{WAITING_COLOR}>{} {ARROW} {}<>",
            shown(row, row.current),
            shown(row, staged)
        ),
        (None, None, None) => String::new(),
    };
    ctx.ui_set_text(&format!("{}.note", row.path), &note);
}

// Paints `row`'s Protected pill as its limit has it.
fn paint_pill(ctx: &mut StableClient<'_>, row: &Row) {
    let ((fill, ink), word) = if row.limit.protected {
        (PILL_ON, said("settings_on", "On"))
    } else {
        (PILL_OFF, said("settings_off", "Off"))
    };
    ctx.ui_set_properties(&format!("{}.pill", row.path), &format!("color: {fill};"));
    let word_path = format!("{}.auto_text", row.path);
    ctx.ui_set_properties(&word_path, &format!("color: {ink};"));
    ctx.ui_set_text(&word_path, &word);
}

// Says in the footer how the manual patch stands: that one is queued, or
// how many numbers wait for the next. Or what an import or an export did,
// while there is that to say.
fn write_status(ctx: &mut StableClient<'_>, window: &mut Window) {
    let queued = crate::patches::hotfix_queued() && window.notice.is_none();
    let waiting = crate::patches::settings(|state| state.staged_count()).unwrap_or(0);
    let status = if let Some(notice) = &window.notice {
        notice.clone()
    } else if queued {
        format!(
            "<{QUEUED_COLOR}>{}<>",
            said(
                "hotfix_note",
                "A manual patch has been queued. Numbers changed while a patch is queued will be honored until the hotfix lands."
            )
        )
    } else if waiting == 0 {
        said("settings_none", "No new values waiting.")
    } else {
        format!(
            "<{WAITING_COLOR}>{}<>",
            said_of(
                "settings_staged",
                "{N} new value(s) waiting for the next manual item patch.",
                &waiting.to_string(),
            )
        )
    };
    if status != window.status {
        // A queued patch has a check ahead of its line, which makes room.
        // The line has the footer as far as the Back button, and goes over
        // two lines where it needs them: a queued patch's is a long one.
        let line = window.under(".popup.footer.status");
        ctx.ui_set_visible(&window.under(".popup.footer.status_icon"), queued);
        ctx.ui_set_properties(&line, if queued { "x: 320px;" } else { "x: 290px;" });
        ctx.ui_set_text(&line, &status);
        window.status = status;
    }
}

// The queue button's look as one number: see [`QUEUE_READY`].
fn queue_look() -> u8 {
    if crate::patches::hotfix_queued() {
        QUEUE_QUEUED
    } else if crate::patches::hotfix_ready() {
        QUEUE_READY
    } else {
        QUEUE_EMPTY
    }
}

// Keeps the footer's queue button as the queue stands, and the line on the
// footer's left with it. Whether anything was painted.
//
// The button is two nodes in one place, as in the editor this is drawn
// after: the button itself, shown while a patch can be queued, and a
// greyed-out box with the same words, or "Queued", shown while none can. So
// nothing is ever written to the button, which is what an event with no
// click behind it would answer.
fn paint_queue(ctx: &mut StableClient<'_>, window: &mut Window, frame: u32) -> bool {
    let look = queue_look();
    if look == window.queue_look {
        return false;
    }
    window.queue_look = look;
    window.queue_at = frame;
    ctx.ui_set_visible(&window.under(".popup.footer.queue"), look == QUEUE_READY);
    ctx.ui_set_visible(&window.under(".popup.footer.queue_off"), look != QUEUE_READY);
    let words = if look == QUEUE_QUEUED {
        said("settings_queued", "Queued")
    } else {
        said("settings_queue", "Queue Manual Patch")
    };
    ctx.ui_set_text(&window.under(".popup.footer.queue_off.text"), &words);
    true
}

// Paints the footer's toggle as it stands.
fn paint_only(ctx: &mut StableClient<'_>, window: &Window) {
    let (fill, ink) = if window.only { PILL_ON } else { PILL_OFF };
    ctx.ui_set_properties(
        &window.under(".popup.footer.only_fill"),
        &format!("color: {fill};"),
    );
    ctx.ui_set_properties(
        &window.under(".popup.footer.only_text"),
        &format!("color: {ink};"),
    );
}

// Shows the entries of the list that match the search box, and with the
// footer's toggle on only those with a number waiting for a patch, one
// under the other. Each is placed outright: nothing is left to a layout's
// own stacking, which is not known to pass over a hidden node.
fn lay_list(ctx: &mut StableClient<'_>, window: &Window) {
    let wanted = window.filter.trim().to_lowercase();
    // The tiers with a number waiting, where the toggle asks for them.
    let waiting: Option<Vec<String>> = window
        .only
        .then(|| crate::patches::settings(|state| state.staged.keys().cloned().collect()))
        .flatten();
    let list = window.list();
    let mut y = 0;
    for (index, entry) in window.entries.iter().enumerate() {
        let node = format!("{list}.i{index}");
        let changed = waiting
            .as_ref()
            .is_none_or(|waiting| entry.keys.iter().any(|key| waiting.contains(key)));
        let matches = changed && (wanted.is_empty() || entry.folded.contains(&wanted));
        ctx.ui_set_visible(&node, matches);
        if matches {
            ctx.ui_set_properties(&node, &format!("y: {y}px;"));
            y += ENTRY_HEIGHT + ENTRY_GAP;
        }
    }
    ctx.ui_set_properties(&list, &format!("height: {}px;", y.max(1)));
}

// Puts the page of the item at `index` of the list up: a card a tier, in
// place of whatever page was up.
fn show_page(ctx: &mut StableClient<'_>, window: &mut Window, index: usize, frame: u32) {
    let page = window.page();
    for card in 0..window.cards {
        ctx.ui_remove_node(&format!("{page}.c{card}"));
    }
    window.cards = 0;
    window.rows.clear();
    window.paged_at = frame;
    let list = window.list();
    if let Some(before) = window.picked.replace(index) {
        ctx.ui_set_visible(&format!("{list}.i{before}.hl"), false);
    }
    ctx.ui_set_visible(&format!("{list}.i{index}.hl"), true);
    let Some(entry) = window.entries.get(index) else {
        return;
    };
    if let Ok(mut last) = LAST_ITEM.lock() {
        *last = Some(entry.id.clone());
    }
    ctx.ui_set_text(&window.under(".popup.main.title"), &entry.name);
    let Some(family) = base::base().families.get(&entry.id) else {
        return;
    };

    // Every tier's rows as the save has them, with the boxes still to fill:
    // (the tier, then for each number what it is called and its row).
    struct Drafted {
        label: String,
        unpatched: f64,
        current: f64,
        whole: bool,
        unsigned: bool,
        percent: bool,
        staged: Option<f64>,
        limit: Limit,
        field: String,
    }
    let drafted: Option<Vec<(String, Vec<Drafted>)>> = crate::patches::settings(|state| {
        family
            .members
            .iter()
            .map(|member| {
                let rows: Vec<Drafted> = ordered(member)
                    .into_iter()
                    .filter_map(|field| {
                        let rule = fields::rule_for(&member.key, field)?;
                        let unpatched = *member.values.get(field)?;
                        let current = balance::reads(state, member, field)?;
                        let (label, percent) = text::field_label(&member.key, field);
                        Some(Drafted {
                            label,
                            unpatched,
                            current,
                            whole: member.whole.contains(field),
                            unsigned: rule.unsigned,
                            percent,
                            staged: state
                                .staged
                                .get(&member.key)
                                .and_then(|staged| staged.get(field))
                                .copied(),
                            limit: state.limit(&member.key, field),
                            field: field.clone(),
                        })
                    })
                    .collect();
                (member.key.clone(), rows)
            })
            .collect()
    });
    let Some(drafted) = drafted else {
        return;
    };

    let off = said("settings_off", "Off");
    let mut y = 0;
    for (card, (key, numbers)) in drafted.into_iter().enumerate() {
        let mut rows_source = String::new();
        for (at, drafted) in numbers.iter().enumerate() {
            let (low, high) = band(drafted.unpatched, drafted.whole);
            let percent = if drafted.percent { "%" } else { "" };
            // What it is called, and under that the config field it is,
            // which is what tells two numbers of one amount apart.
            let label = if drafted.label.is_empty() {
                drafted.field.clone()
            } else {
                drafted.label.clone()
            };
            let sub = format!(
                "{} ({})",
                drafted.field,
                said_of("settings_default", "default {N}", &number(drafted.unpatched))
            );
            rows_source.push_str(&fill(
                FIELD_SOURCE,
                &[
                    ("%N%", at.to_string().as_str()),
                    ("%Y%", (CARD_HEAD + at as u32 * FIELD_HEIGHT).to_string().as_str()),
                    ("%LABEL%", clean(&label).as_str()),
                    ("%SUB%", clean(&sub).as_str()),
                    ("%CURRENT%", format!("{}{percent}", number(drafted.current)).as_str()),
                    ("%LOW%", number(low).as_str()),
                    ("%HIGH%", number(high).as_str()),
                    ("%OFF%", off.as_str()),
                ],
            ));
        }
        let height = CARD_HEAD + numbers.len() as u32 * FIELD_HEIGHT + CARD_FOOT;
        let source = fill(
            CARD_SOURCE,
            &[
                ("%ROWS%", rows_source.as_str()),
                ("%N%", card.to_string().as_str()),
                ("%Y%", y.to_string().as_str()),
                ("%HEIGHT%", height.to_string().as_str()),
                ("%SHEET%", ITEM_SHEET),
                ("%FRAME%", clean(&text::item_frame(&key)).as_str()),
                ("%NAME%", clean(&text::item_name(&key)).as_str()),
                ("%H_SETTING%", said("settings_col_setting", "Setting").as_str()),
                ("%H_CURRENT%", said("settings_col_current", "Current").as_str()),
                ("%H_VALUE%", said("settings_col_new", "New Value").as_str()),
                ("%H_MIN%", said("settings_col_min", "Minimum").as_str()),
                ("%H_MAX%", said("settings_col_max", "Maximum").as_str()),
                ("%H_PROTECTED%", said("settings_col_protected", "Protected").as_str()),
            ],
        );
        if !ctx.ui_spawn_source(&page, &source) {
            break;
        }
        window.cards = card + 1;
        y += height + CARD_GAP;
        for (at, drafted) in numbers.into_iter().enumerate() {
            let path = format!("{page}.c{card}.f{at}");
            let boxed = |limit: Option<f64>| limit.map(number).unwrap_or_default();
            let row = Row {
                value: TextBox::filled(
                    ctx,
                    format!("{path}.value"),
                    drafted.staged.map(number).unwrap_or_default(),
                ),
                min: TextBox::filled(ctx, format!("{path}.min"), boxed(drafted.limit.min)),
                max: TextBox::filled(ctx, format!("{path}.max"), boxed(drafted.limit.max)),
                path,
                key: key.clone(),
                field: drafted.field,
                unpatched: drafted.unpatched,
                current: drafted.current,
                whole: drafted.whole,
                unsigned: drafted.unsigned,
                percent: drafted.percent,
                staged: drafted.staged,
                limit: drafted.limit,
                value_error: None,
                limit_error: None,
            };
            ctx.ui_register_path_events(&format!("{}.auto", row.path), handle_click);
            if row.limit.protected {
                paint_pill(ctx, &row);
            }
            if row.staged.is_some() {
                write_note(ctx, &row);
            }
            window.rows.push(row);
        }
    }
    // The scroll goes as far as the cards do.
    ctx.ui_set_properties(&page, &format!("height: {}px;", y.max(1)));
}

// Opens the window over the dialog at `dialog`. Nothing where it cannot be
// spawned, or no save's patch state is there to show.
fn open(ctx: &mut StableClient<'_>, dialog: &str, frame: u32) -> Option<Window> {
    crate::patches::settings(|_| ())?;
    // Under the layout root the dialog is in, and failing that beside the
    // dialog, under the root of its own layout: the window centres itself
    // on whichever it is given.
    let layout = dialog.split('.').next().filter(|root| !root.is_empty())?;
    let mut parents = vec![layout];
    if let Some((beside, _)) = dialog.rsplit_once('.') {
        if beside != layout {
            parents.push(beside);
        }
    }
    // One left from an earlier opening is not the last child any more if
    // game code has built its dialog again since.
    for parent in &parents {
        let left = format!("{parent}.{WINDOW}");
        if ctx.ui_exists(&left) {
            ctx.ui_remove_node(&left);
        }
    }
    let subtitle = said(
        "settings_subtitle",
        "New values are released by the next manual item patch. Minimum, Maximum and Protected only apply to automatic patches. Empty a box to go back to the default.",
    );
    let source = fill(
        WINDOW_SOURCE,
        &[
            ("%NODE%", WINDOW),
            ("%TITLE%", said("settings_row", "Item Patcher Settings").as_str()),
            ("%SEARCH%", said("settings_search", "Search items...").as_str()),
            ("%SUBTITLE%", subtitle.as_str()),
            ("%ONLY%", said("settings_only", "Changed Items Only").as_str()),
            ("%BACK%", said("settings_back", "Back").as_str()),
            ("%QUEUE%", said("settings_queue", "Queue Manual Patch").as_str()),
            ("%IMPORT%", said("settings_import", "Import").as_str()),
            (
                "%IMPORT_HINT%",
                said(
                    "settings_import_hint",
                    "Replace this save's settings with the ones in a file you choose.",
                )
                .as_str(),
            ),
            ("%EXPORT%", said("settings_export", "Export").as_str()),
            (
                "%EXPORT_HINT%",
                said(
                    "settings_export_hint",
                    "Write this save's settings to a file that other saves can import.",
                )
                .as_str(),
            ),
        ],
    );
    let parent = parents
        .into_iter()
        .find(|parent| ctx.ui_spawn_source(parent, &source))?;
    let root = format!("{parent}.{WINDOW}");

    let mut entries: Vec<Entry> = base::base()
        .families
        .iter()
        .filter_map(|(id, family)| {
            let lead = family.members.first()?;
            let name = text::item_name(&lead.key);
            Some(Entry {
                id: id.clone(),
                keys: family.members.iter().map(|member| member.key.clone()).collect(),
                folded: name.to_lowercase(),
                frame: text::item_frame(&lead.key),
                name,
            })
        })
        .collect();
    entries.sort_by(|a, b| a.folded.cmp(&b.folded));

    let mut window = Window {
        root,
        entries,
        filter: String::new(),
        only: false,
        queue_look: 0,
        queue_at: frame,
        picked: None,
        cards: 0,
        rows: Vec::new(),
        opened_at: frame,
        paged_at: frame,
        status: String::new(),
        notice: None,
    };
    let list = window.list();
    for (index, entry) in window.entries.iter().enumerate() {
        let source = fill(
            ENTRY_SOURCE,
            &[
                ("%N%", index.to_string().as_str()),
                ("%Y%", (index as u32 * (ENTRY_HEIGHT + ENTRY_GAP)).to_string().as_str()),
                ("%SHEET%", ITEM_SHEET),
                ("%FRAME%", clean(&entry.frame).as_str()),
                ("%NAME%", clean(&entry.name).as_str()),
            ],
        );
        if !ctx.ui_spawn_source(&list, &source) {
            break;
        }
        ctx.ui_register_path_events(&format!("{list}.i{index}.sel"), handle_click);
    }
    lay_list(ctx, &window);
    for node in [
        ".popup.close",
        ".popup.import",
        ".popup.export",
        ".popup.footer.back",
        ".popup.footer.only",
        ".popup.footer.queue",
    ] {
        ctx.ui_register_path_events(&window.under(node), handle_click);
    }
    paint_queue(ctx, &mut window, frame);

    // On the item it was last closed on, or on the first.
    let last = LAST_ITEM.lock().ok().and_then(|last| last.clone());
    let first = last
        .and_then(|id| window.entries.iter().position(|entry| entry.id == id))
        .unwrap_or(0);
    if !window.entries.is_empty() {
        show_page(ctx, &mut window, first, frame);
    }
    write_status(ctx, &mut window);
    Some(window)
}

// -- events ---------------------------------------------------------------------

// A click on the row's button asks for the window: on a button that is on
// screen and has been for a moment ([`SETTLE_FRAMES`]). An unreported kind
// counts as a click: a window opened for nothing is closed again, and a
// button that opens nothing is the worse failure.
fn handle_open(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    if !event.path.ends_with(BUTTON_UNDER) {
        return;
    }
    let since = FRAME
        .load(Ordering::Relaxed)
        .wrapping_sub(ROW_SETTLED_AT.load(Ordering::Relaxed));
    if matches!(event.kind, Some(UiEventKindV1::Click) | None)
        && ROW_SHOWN.load(Ordering::Relaxed)
        && since >= SETTLE_FRAMES
    {
        OPEN.store(true, Ordering::Relaxed);
    }
}

// A click on a node of the window is noted for the next frame. An
// unreported kind counts as a click, the gate `item_stats::ui` uses.
fn handle_click(ctx: &mut StableClient<'_>) {
    let Some(event) = ctx.ui_current_event() else {
        return;
    };
    if !matches!(event.kind, Some(UiEventKindV1::Click) | None) {
        return;
    }
    if let Ok(mut clicks) = CLICKS.lock() {
        if !clicks.contains(&event.path) {
            clicks.push(event.path);
        }
    }
}

// -- every frame ----------------------------------------------------------------

// Keeps the row in the dialog and on its tab.
//
// It is spawned when the dialog has none, which is once for each time game
// code builds the dialog: the dialog's own rows are all there from the
// start, so a row appended then is the last and stays the last. Its button
// is registered then, and again each time the dialog opens: in the pause
// menu's layout a registration does not outlast the dialog closing or the
// game being saved, and a button registered once goes dead.
fn sync_row(ctx: &mut StableClient<'_>, dialog: Option<&str>, frame: u32) {
    let Some(dialog) = dialog else {
        ROW_UP.store(false, Ordering::Relaxed);
        ROW_SHOWN.store(false, Ordering::Relaxed);
        return;
    };
    let opened = !ROW_UP.swap(true, Ordering::Relaxed);
    let contents = format!("{dialog}.contents");
    let row = format!("{contents}.{ROW}");
    let fresh = !ctx.ui_exists(&row);
    if fresh {
        let worded =
            |words: (&str, &str)| options_dialog::escape(&options_dialog::text(ctx, words));
        let source = fill(
            ROW_SOURCE,
            &[
                ("%NODE%", ROW),
                ("%TITLE%", worded(ROW_TITLE).as_str()),
                ("%OPEN%", worded(ROW_OPEN).as_str()),
                ("%HINT%", worded(ROW_HINT).as_str()),
            ],
        );
        if !ctx.ui_spawn_source(&contents, &source) {
            return;
        }
    }
    if fresh || opened {
        ctx.ui_register_path_events(&format!("{row}{BUTTON_UNDER}"), handle_open);
        ROW_SETTLED_AT.store(frame, Ordering::Relaxed);
    }
    let competition = options_dialog::competition_up(ctx, dialog);
    ctx.ui_set_visible(&row, competition);
    if competition && !ROW_SHOWN.swap(true, Ordering::Relaxed) {
        ROW_SETTLED_AT.store(frame, Ordering::Relaxed);
    }
    if !competition {
        ROW_SHOWN.store(false, Ordering::Relaxed);
    }
}

// Opens the window when its button was clicked, and while it is up reads
// its clicks and its boxes. It closes on its cross and on Back, and with the
// dialog under it.
fn sync_window(ctx: &mut StableClient<'_>, dialog: Option<&str>, frame: u32) {
    // A file dialog that has closed is acted on whether the window is up or
    // not: a file chosen is never written or read late.
    let picked = transfer::finished();
    let wanted = OPEN.swap(false, Ordering::Relaxed);
    let clicks = CLICKS
        .lock()
        .map(|mut clicks| std::mem::take(&mut *clicks))
        .unwrap_or_default();
    let Ok(mut opened) = OPENED.lock() else {
        return;
    };
    if opened.is_none() {
        if let (true, Some(dialog)) = (wanted, dialog) {
            *opened = open(ctx, dialog, frame);
        }
        return;
    }
    let Some(window) = opened.as_mut() else {
        return;
    };
    let close = window.under(".popup.close");
    let back = window.under(".popup.footer.back");
    let settled = frame.wrapping_sub(window.opened_at) >= SETTLE_FRAMES;
    let closed = settled && clicks.iter().any(|path| *path == close || *path == back);
    // Gone with the pause menu, left behind by a dialog that has closed, or
    // closed by the player.
    if closed || dialog.is_none() || !ctx.ui_exists(&window.root) {
        ctx.ui_remove_node(&window.root);
        *opened = None;
        return;
    }

    // The search box.
    if let Some(filter) = ctx.ui_text_edit_text(&window.under(".popup.sidebar.search")) {
        if filter != window.filter {
            window.filter = filter;
            lay_list(ctx, window);
        }
    }

    // The footer's toggle.
    let only = window.under(".popup.footer.only");
    if settled && clicks.iter().any(|path| *path == only) {
        window.only = !window.only;
        paint_only(ctx, window);
        lay_list(ctx, window);
    }

    // The item list: the last entry clicked is the one picked.
    let list = format!("{}.i", window.list());
    let picked = clicks.iter().rev().find_map(|path| {
        path.strip_prefix(list.as_str())?
            .strip_suffix(".sel")?
            .parse::<usize>()
            .ok()
    });
    if let Some(index) = picked.filter(|_| settled) {
        if window.picked != Some(index) && index < window.entries.len() {
            show_page(ctx, window, index, frame);
        }
    }

    // The Protected pills, unless the page they were clicked on has gone.
    let mut changed = false;
    if frame.wrapping_sub(window.paged_at) >= SETTLE_FRAMES {
        for row in &mut window.rows {
            let pill = format!("{}.auto", row.path);
            if clicks.iter().any(|path| *path == pill) {
                row.limit.protected = !row.limit.protected;
                keep_limit(row);
                paint_pill(ctx, row);
                changed = true;
            }
        }
    }

    // The boxes.
    for row in &mut window.rows {
        let mut typed_into = false;
        if let Some(typed_text) = row.value.typed(ctx) {
            take_value(row, &typed_text);
            typed_into = true;
        }
        let min = row.min.typed(ctx).is_some();
        let max = row.max.typed(ctx).is_some();
        if min || max {
            take_limits(row);
            typed_into = true;
        }
        if typed_into {
            write_note(ctx, row);
            changed = true;
        }
    }
    // What an import or an export said is old news once anything changes.
    if changed {
        window.notice = None;
    }

    // Import and Export each open a file dialog, which the footer points to
    // while it is up. Once a file is chosen it is written or read
    // (`transfer::finished`), and an import puts other numbers in the boxes,
    // so the page is made again from what the save has now.
    let export = window.under(".popup.export");
    if settled && clicks.iter().any(|path| *path == export) {
        if let Some(notice) = transfer::start_export() {
            window.notice = Some(notice);
            changed = true;
        }
    }
    let import = window.under(".popup.import");
    if settled && clicks.iter().any(|path| *path == import) {
        if let Some(notice) = transfer::start_import() {
            window.notice = Some(notice);
            changed = true;
        }
    }
    if let Some(done) = picked {
        if done.imported {
            if let Some(index) = window.picked {
                show_page(ctx, window, index, frame);
            }
        }
        window.notice = done.notice;
        changed = true;
    }

    // A number set or taken back can put its item into the narrowed list,
    // or take it out.
    if changed && window.only {
        lay_list(ctx, window);
    }

    // The queue button: a click on it queues the patch, where it has been
    // showing as one that can be for a moment. One event queues a patch,
    // with nothing to confirm it, so the event has to be the player's click
    // and no other.
    let queue = window.under(".popup.footer.queue");
    if settled
        && window.queue_look == QUEUE_READY
        && frame.wrapping_sub(window.queue_at) >= SETTLE_FRAMES
        && clicks.iter().any(|path| *path == queue)
    {
        crate::patches::queue_hotfix();
        window.notice = None;
    }
    // As the queue stands now: it empties under an open window when its
    // patch goes out, and what can be queued follows what has been typed.
    if paint_queue(ctx, window, frame) || changed {
        write_status(ctx, window);
    }
}

// Every client frame. With the dialog closed and no window up this is one
// lookup, two locks and an atomic read.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    // No settings where there are no patches.
    if !crate::patches::hotfixes_offered() {
        return;
    }
    let frame = FRAME.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
    let dialog = options_dialog::showing(ctx, frame);
    sync_row(ctx, dialog.as_deref(), frame);
    sync_window(ctx, dialog.as_deref(), frame);
}
