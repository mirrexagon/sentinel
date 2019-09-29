## Commands
- .talk like ( me | <@mention> ) - generate text from a user's speakings
- .speak like ( me | <@mention> ) - same as above but TTS
- .clear my talk data - clear a user's talk data

## Implementation notes
- Check that generated messages are between 0 and 2000 Unicode code points in length. Serenity returns a special error if you try and send a too-long message.
- Content safe for mentions? Serenity has a thing for this

## Ideas
- Be able to specify a seed word
- Be able to specify minimum generated message length
