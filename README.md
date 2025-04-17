# Website

A self contained website creator and server with zero conf simplicity.

## Philosophy

Simplicity, you should be able to self host websites without extensive
configuration setups and in a safe way.

All batteries included, all functionality is embedded into the program, data
stores, search indexing, authentication.

As simple to run locally as in production, there should not be a need to
complicate either, solving complexity is the job of the program.

## Installing and running

For now there are no pre-built binary distrubutions. You will need to have [rust
and cargo installed](https://www.rust-lang.org/learn/get-started) on your
system.

```bash
$ cargo install --git https://github.com/tirithen/website.git
$ website
```

That is it, this will start a web server and you are ready to create page
content. The program will instruct you where it reads the content from.

## Early days

These are early days, for now mostly basic page serving and search is currently
working. See [TODO.md](TODO.md) for the current list of ideas.

The service has been developed on Linux, other OSes remain largely untested,
any help in that area would be appreciated.

## Contributing

You can help out in several ways:

1. Try out the crate
2. Report any issues, bugs, missing documentation or examples
3. Create issues with feedback on the ergonomy of the program
4. Extend the documentation or examples
5. Contribute code changes

Feedback on the ergonomics of this service or its features/lack there of might
be as valuable as code contributions.

### Code contributions

Code contributions are more than welcome. Creating a pull-request is more useful
than issues with feature requests.

Once a pull request is ready for merge, also squash the commits into a single
commit per feature or fix.

The commit messages in this project should comply with the
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) format
so that the [semver](https://semver.org/) versions can be automatically
calculated and the release changelog can be automatically generated.
