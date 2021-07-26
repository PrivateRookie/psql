# psql
parameterized SQL parser

## syntax

`--?` stands for param definition, format

`--? <name>: <ty> [= <default>] [// <help message>]`

now PSQL support 3 basic ty `str`, `num`, `raw`,

`str` can be wrapped by double quote or single quote, `num` should be valid f64 in rust, and
`raw` stands for insert what ever you passwd, so, you can use it to insert build in function like `Date()`, `raw` is wrapped by "#".

Psql supports array type, format: `[<basic_type>]`

You can set a default value for parameter and help message, they are optional.

Use `@name` format to replace with parameter.

demo

```sql
--? age: num = 10 // useful help message
--? pattern: str // help
--? addrs: [str] = ['sh', 'beijing'] // address
--? pp: [num] // 必须使用???
select name from t where age=@age and name like @pattern and addr in @addrs and scores in @pp
```

to see usage, check [examples](./examples).

