//go:build !linux

package main

import "os/exec"

// Elsewhere the crash round kills the writer alone; it is meant for Linux.
func ownGroup(*exec.Cmd) {}

func killGroup(cmd *exec.Cmd) { _ = cmd.Process.Kill() }
