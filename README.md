# fastcdc-diff

> A tool that uses FastCDC algorithm to effectively split binary data and generate the diff.

## Usage

```javascript
const { diff, apply } = require('fastcdc-diff');

diff('A.bin', 'B.bin', 'a-b.diff');
apply('a-b.diff', 'A.bin', 'newB.bin');
```
