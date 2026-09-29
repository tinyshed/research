package main

import (
	"os/exec"
	"syscall"
)

// ownGroup puts the writer and every server it starts in a process group of
// their own, so that one signal kills them all
func ownGroup(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}

func killGroup(cmd *exec.Cmd) {
	_ = syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
}
