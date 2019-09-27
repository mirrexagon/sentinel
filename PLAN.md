## Commands
- .talk like ( me | <@mention> ) - generate text from a user's speakings
- .speak like ( me | <@mention> ) - same as above but TTS
- .clear my talk data - clear a user's talk data

## Implementation notes
- Check that generated messages are between 0 and 2000 bytes in length.
- Try only a limited number of times to generate messages

## Ideas
- Be able to specify a seed word
- Be able to specify minimum generated message length
- Increase multi message limit (how many messages can be generated in once "talk like" call)
- Re-enable self-learning
