#!/bin/sh

exec cargo fmt -- --config group_imports=StdExternalCrate --config imports_granularity=Crate $@
