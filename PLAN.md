## Commands
- .talk like/.t ( me | <@mention> ) [num up to 5] - generate text from a user's past messages, up to num many texts
- .speak like/.s ( me | <@mention> ) [num up to 5] - same as above but TTS
- .clear my data - clear a user's talk data

## Implementation notes
- Check that generated messages are between 0 and 2000 Unicode code points in length. Serenity returns a special error if you try and send a too-long message.
- Content safe for mentions? Serenity has a thing for this

## Ideas
- Be able to specify a seed word
- Be able to specify minimum generated message length

- Flakeify: https://www.reddit.com/r/rust/comments/mmbfnj/nixifying_a_rust_project/

## Bugs
Don't clear data on sentinel unless there are no args

Cool down on mimic

Update help text for new command

Fix no message when making message when cleared - only need to delete chain, will be remade when user says something
