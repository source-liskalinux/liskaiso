#!/bin/bash
# Liska Linux banner and terminal configuration

# Format: [ username@hostname ] / # sudo foo
PROMPT="%B%F{cyan}[ %n@%m ]%f %F{lightgreen}%~%f %F{white}>%f%b "
CYAN='\033[36m'; GREEN='\033[92m'; BOLD='\033[1m'; RESET='\033[0m'
printf "${BOLD}----------------------------${RESET}\n"
printf "${CYAN}${BOLD}::: [ LISKA LINUX LIVE ] :::${RESET}\n"
printf "${BOLD}----------------------------${RESET}\n\n"
printf "Welcome to ${CYAN}${BOLD}Liska Linux${RESET} live iso!\n"
printf "Installation guide page:\n"
printf "${GREEN}${BOLD}https://liskalinuxwiki.web.app/installation-guide/${RESET}\n\n"
printf "To connect to a Wi-Fi network, use ${CYAN}nmtui${RESET} utility.\n"
printf "For mobile broadband (WWAN) modems, use ${CYAN}mmcli${RESET} utility.\n"
printf "Ethernet, WLAN and WWAN interfaces using DHCP should work automatically.\n\n"
printf "──────────────────────────────────────────────────────────────────────────────\n\n"
