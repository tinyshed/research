//go:build !linux

package main

import "errors"

func syncAll() error { return errors.New("dropping the page cache is Linux's") }
