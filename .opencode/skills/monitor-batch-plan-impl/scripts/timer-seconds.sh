#!/bin/bash
# timer-seconds.sh — vestigial: the peak-strategy boundaries are gone
# (one model pair, no window — see models.jsonc and the monitor SKILL).
# Kept as a stub so any stale caller still gets a sane answer:
# prints "<interval> none".
set -u
echo "${1:-1200} none"
