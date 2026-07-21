#!/bin/bash
# Liska Linux - Dynamic Wallpaper Switcher (Desktop + SDDM Theme Sync)

WALLPAPER_DIR="$HOME/Pictures/Wallpapers"
mkdir -p "$WALLPAPER_DIR"

if [ -z "$(ls -A "$WALLPAPER_DIR" 2>/dev/null)" ]; then
curl -s -L "https://raw.githubusercontent.com/liskalinux/assets/main/wallpapers/glassy-cyan.jpg" -o "$WALLPAPER_DIR/glassy-cyan.jpg"
fi

SELECTED_FILE=$(zenity --file-selection --title="Select Wallpaper" --filename="$WALLPAPER_DIR/")

if [ -n "$SELECTED_FILE" ]; then
cat <<EOF > ~/.config/hypr/hyprpaper.conf
preload = $SELECTED_FILE
wallpaper = ,$SELECTED_FILE
ipc = off
EOF
pkill hyprpaper
hyprpaper &
SDDM_TARGET="/usr/share/sddm/themes/liska-glassy/background.jpg"    
notify-send "System" "Updating Wallpaper on Desktop and SDDM...." -i preferences-desktop-wallpaper
pkexec sh -c "mkdir -p /usr/share/sddm/themes/liska-glassy && cp '$SELECTED_FILE' '$SDDM_TARGET' && chmod 644 '$SDDM_TARGET'"
notify-send "System" "Wallpaper has been updated successfully!" -i preferences-desktop-wallpaper
fi