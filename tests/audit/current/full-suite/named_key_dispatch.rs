use remacs::lisp::Interp;
fn main() {
 std::thread::Builder::new().stack_size(64<<20).spawn(|| {
  let mut i=Interp::new();
  i.eval_str("(progn (erase-buffer) (insert \"alpha\\nbeta\") (goto-char 9))").unwrap();
  let binding=i.eval_str("(key-binding [C-home])").unwrap();
  println!("C-home binding: {}",i.prin1_to_string(&binding));
  let h="home".bytes().fold(0usize,|h,b|h.wrapping_mul(31).wrapping_add(b as usize));
  let base=0x4000_0000i128+(h as i128 & 0x1f_ffff);
  let mut keys=Vec::new();let mut arg=false;
  remacs::term::dispatch_key_core(&mut i,base|0x0400_0000,&mut keys,&mut arg,None).unwrap();
  let p=i.eval_str("(list (point) (current-message))").unwrap();println!("C-home actual: {}",i.prin1_to_string(&p));
  keys.clear();
  remacs::term::dispatch_key_core(&mut i,base,&mut keys,&mut arg,None).unwrap();
  let p=i.eval_str("(point)").unwrap();println!("Home control: {}",i.prin1_to_string(&p));
 }).unwrap().join().unwrap();
}
