package main

import "syscall"

func syncAll() error {
	syscall.Sync()
	return nil
}
