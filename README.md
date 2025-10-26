## payments-toy-engine

### Implementation
Event sourcing based implementation is used, backed with sqlite database.

Sqlite db is used for storing aggregate events, snapshots, as well as resulting account projections.

2 aggregate types are defined: `Account` and `Transaction`.

`Account` is used for maintaining balance and state, while `Transaction` is used for recording and tracking transactions, for not allowing duplicates, and retrieving amount by transaction id when there is a dispute action raised.

`PaymentService` is used as an entry point for taking csv row input and orchestrating operations between those 2 aggregates.
It's implementation is quite naive, but could be turned into SAGA like thing for a production readiness.

All the processing is implemented in a way where one process (`sender`) reads all the csv rows and publishes/distributes to specific `receivers` which are pinned to some client id (like consumer groups in Kafka).
Those receivers then initiate `PaymentService` steps.
This parallel processing logic in [main](src/main.rs).

Key code sections:
* [Account Command](src/domain/account/command.rs#L6)
* [Account Handlers](src/domain/account/aggregate.rs#L87)
* [Account Event Appliers](src/domain/account/aggregate.rs#L61)
* [Account Tests](src/domain/account/aggregate.rs#L268)
* [Transaction Command](src/domain/transaction/command.rs#L6)
* [Transaction Handler](src/domain/transaction/aggregate.rs#L58)
* [Transaction Event Applier](src/domain/transaction/aggregate.rs#L48)
* [Payment Service](src/payments.rs#L67)
* [CLI integration tests](tests/cli.rs#L6)


### Assumptions made (possibly can be aligned with product owner)
* Assuming that in unexpected errors case like - missing or wrong input file, program should fail with non zero exit code and error message.
* Assuming input tx type is a case sensitive (lowercase).
* Assuming we can allow a dispute only when there is enough available funds and only for the `deposit` type transactions.

### Running
```cargo run -- sample/transactions.csv > accounts.csv```

Note: there will be a temp sqlite files generated per run & per cpu core like 'XDB-1761491588862857000-0.db'.