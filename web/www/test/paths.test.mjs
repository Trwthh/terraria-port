import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ERRNO, parsePath, checkWrite, parseForWrite, errnoFromDomError } from '../lib/paths.js';

test('parsePath accepts install and saves paths, normalises separators', () => {
  assert.deepEqual(parsePath('/install/Content/Images/Item_1.xnb'), { parts: ['install', 'Content', 'Images', 'Item_1.xnb'], root: 'install' });
  assert.deepEqual(parsePath('\\saves\\Players\\a.plr'), { parts: ['saves', 'Players', 'a.plr'], root: 'saves' });
  assert.deepEqual(parsePath('//saves//x//'), { parts: ['saves', 'x'], root: 'saves' });
  assert.deepEqual(parsePath('/'), { parts: [], root: null });
});

test('parsePath rejects relative, dot segments, unknown roots, NUL, long names', () => {
  assert.equal(parsePath('saves/x').err, ERRNO.EINVAL);
  assert.equal(parsePath('').err, ERRNO.EINVAL);
  assert.equal(parsePath('/saves/../install/x').err, ERRNO.EINVAL);
  assert.equal(parsePath('/saves/./x').err, ERRNO.EINVAL);
  assert.equal(parsePath('/etc/passwd').err, ERRNO.ENOENT);
  assert.equal(parsePath('/Saves/x').err, ERRNO.ENOENT, 'roots are exact');
  assert.equal(parsePath('/saves/a\0b').err, ERRNO.EINVAL);
  assert.equal(parsePath('/saves/' + 'a'.repeat(256)).err, ERRNO.ENAMETOOLONG);
  assert.equal(parsePath('/saves/' + 'a/'.repeat(3000)).err, ERRNO.ENAMETOOLONG);
  assert.equal(parsePath(42).err, ERRNO.EINVAL);
});

test('write guard: everything under /install is read-only', () => {
  for (const p of ['/install', '/install/Content', '/install/Content/Images/Item_1.xnb', '/install/new.txt', '\\install\\x']) {
    assert.equal(checkWrite(parsePath(p)), ERRNO.EACCES, p);
    assert.equal(parseForWrite(p).err, ERRNO.EACCES, p);
  }
});

test('write guard: roots themselves cannot be modified', () => {
  assert.equal(checkWrite(parsePath('/')), ERRNO.EACCES);
  assert.equal(checkWrite(parsePath('/saves')), ERRNO.EACCES);
  assert.equal(checkWrite(parsePath('/saves/')), ERRNO.EACCES);
});

test('write guard: /saves/... is writable; parse errors pass through', () => {
  assert.equal(checkWrite(parsePath('/saves/Players/a.plr')), 0);
  assert.equal(checkWrite(parsePath('/saves/x')), 0);
  assert.deepEqual(parseForWrite('/saves/Worlds/w.wld'), { parts: ['saves', 'Worlds', 'w.wld'], root: 'saves' });
  assert.equal(checkWrite(parsePath('/saves/../install/x')), ERRNO.EINVAL);
  assert.equal(checkWrite(null), ERRNO.EINVAL);
});

test('DOMException names map to errno', () => {
  assert.equal(errnoFromDomError({ name: 'NotFoundError' }), ERRNO.ENOENT);
  assert.equal(errnoFromDomError({ name: 'NoModificationAllowedError' }), ERRNO.EBUSY);
  assert.equal(errnoFromDomError({ name: 'QuotaExceededError' }), ERRNO.ENOSPC);
  assert.equal(errnoFromDomError(new Error('x')), ERRNO.EIO);
});
