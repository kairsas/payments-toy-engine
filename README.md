## payments-toy-engine

### Running
```cargo run -- transactions.csv > accounts.csv```

### Assumptions (possibly can be aligned with product owner)
* Assuming that unexpected errors like, missing input, will fail with non zero exit code and error message.
* Assuming input tx type is case sensitive
* Assuming we can allow a dispute only when there is enough available funds and only for the deposit transactions

### Possible optimisations
* Parallelise csv row processing by using `rayon` and maybe using some groups by client account id.