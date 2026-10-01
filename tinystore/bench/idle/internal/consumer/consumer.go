package consumer

import (
	"context"
	"errors"

	"github.com/tinyshed/tinystore"
)

type Engine func(context.Context, *tinystore.Store) error

func Open(engines ...Engine) func(context.Context, string) (func(context.Context) error, error) {
	return func(ctx context.Context, dir string) (func(context.Context) error, error) {
		store, err := tinystore.Open(ctx, dir, tinystore.Options{})
		if err != nil {
			return nil, err
		}
		for _, engine := range engines {
			if err = engine(ctx, store); err != nil {
				return nil, errors.Join(err, store.Close(ctx))
			}
		}
		return store.Close, nil
	}
}
