#!/bin/bash
# Liska Linux banner and terminal configuration

# Format: [ username@hostname ] / # sudo foo
PROMPT="%B%F{cyan}[ %n@%m ]%f %F{lightgreen}%~%f %F{white}➤%f%b "
CYAN='\x1b[36m'; GREEN='\x1b[92m'; BOLD='\033[1m'; RESET='\x1b[0m'
printf "──────────────────────────────────────────────────────────────────────────────\n\n"
printf "${CYAN}${BOLD}::: [ LISKA LINUX LIVE ] :::${RESET}\n\n"
printf "Welcome to ${CYAN}Liska Linux${RESET} live iso!\n"
printf "Installation guide page:\n"
printf "${GREEN}https://liskalinux.codeberg.page/installation-guide/${RESET}\n\n"
printf "To connect to a Wi-Fi network, use ${CYAN}nmtui${RESET} utility.\n"
printf "For mobile broadband (WWAN) modems, use ${CYAN}mmcli${RESET} utility.\n"
printf "Ethernet, WLAN and WWAN interfaces using DHCP should work automatically.\n\n"
printf "──────────────────────────────────────────────────────────────────────────────\n\n"
