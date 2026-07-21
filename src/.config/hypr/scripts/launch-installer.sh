#!/bin/bash
# Launch the Calamares installer using pkexec

if [ -f /usr/bin/calamares ]; then
    pkexec calamares -d
else
    notify-send "Liska Linux Installer" "Calamares Installer not found on this system." -i dialog-error
fi