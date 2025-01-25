@echo off
echo updating op funcs
python gen_op_funcs.py
echo updating tests
python create_tests.py