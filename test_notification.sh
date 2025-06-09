#!/usr/bin/env bash

# Test script for notification messages

echo "Testing notification messages..."

# Test completion notification
echo "Testing completion notification:"
say "Agent 12 has completed work"
sleep 2

# Test resume notification
echo "Testing resume notification:"
say "Agent 12 has resumed work"
sleep 2

# Test error notification
echo "Testing error notification:"
say "Agent 12 detected unknown box pattern. Check required."
sleep 2

echo "All notifications tested."