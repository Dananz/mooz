import os

app = os.environ["APP"]                 # absolute path to Mooz.app
application = os.path.basename(app)      # "Mooz.app"

# DMG format: compressed, read-only
format = "UDZO"

# Contents
files = [app]
symlinks = {"Applications": "/Applications"}

# Volume badge icon (the mounted disk icon)
badge_icon = os.environ["ICNS"]

# Window styling
background = os.environ["BG"]
window_rect = ((220, 140), (540, 380))   # ((x, y), (w, h)) — matches background size
default_view = "icon-view"
show_icon_preview = False
icon_size = 128
text_size = 13

icon_locations = {
    application: (140, 196),
    "Applications": (400, 196),
}
