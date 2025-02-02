@echo off
python gen_op_funcs.py && cargo run --release -- nestest.nes --trace --endonbrk --pc C000 & python check_trace.py