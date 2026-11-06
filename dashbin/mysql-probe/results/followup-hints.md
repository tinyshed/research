# optimizer hints on MySQL 8.4, through its own client in the container

`SELECT /*+ SET_VAR(sql_mode='NO_BACKSLASH_ESCAPES') */ 'x\' AS a -- '` as root:

    x' AS a --

the same without the hint:

    x' AS a --

`SET SESSION max_execution_time = 1000; SELECT /*+ SET_VAR(max_execution_time = 600000) */ COUNT(*) FROM probe.big a,
probe.big b WHERE a.amount + b.amount < 0` as reader, killed by root after 4 s:

    still running after 4 s (time=4), killing 73
    ERROR 1317 (70100) at line 1: Query execution was interrupted
    after 4192 ms
