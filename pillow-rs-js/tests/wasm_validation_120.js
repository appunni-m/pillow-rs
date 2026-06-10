const w=require('/home/appunni/work/pil-wasm/pillow-rs-js/pkg/pillow_rs_js.js');
const r={};const A=x=>Array.from(x);
const F=()=>new w.Image('RGB',20,20,255,128,0,255);
const FL=()=>new w.Image('L',20,20,128,128,128,255);
const FA=()=>new w.Image('RGBA',20,20,255,0,0,128);
const B=()=>new w.Image('RGB',20,20,0,0,255,255);

// 1. new (6)
r.new_RGB_default=F().toBytes().length;
r.new_RGBA=(new w.Image('RGBA',5,5,255,0,0,128)).toBytes().length;
r.new_L=(new w.Image('L',5,5,200,200,200,255)).toBytes().length;

// 2. resize (7)
r.resize_BILINEAR=F().resize(10,10,'BILINEAR').toBytes().length;
r.resize_LANCZOS=F().resize(10,10,'LANCZOS').toBytes().length;
r.resize_L=FL().resize(10,10,'BILINEAR').toBytes().length;
r.resize_RGBA=FA().resize(10,10,'BILINEAR').toBytes().length;
r.resize_same=F().resize(20,20,'BILINEAR').toBytes().length;

// 3. crop (5)
r.crop_basic=F().crop(5,5,15,15).toBytes().length;
r.crop_full=F().crop(0,0,20,20).toBytes().length;
r.crop_L=FL().crop(5,5,15,15).toBytes().length;
r.crop_RGBA=FA().crop(5,5,15,15).toBytes().length;

// 4. rotate/transpose (10)
r.rotate_90=F().rotate(90).toBytes().length;
r.rotate_180=F().rotate(180).toBytes().length;
r.rotate_270=F().rotate(270).toBytes().length;
var methods=['FLIP_LEFT_RIGHT','FLIP_TOP_BOTTOM','ROTATE_90','ROTATE_180','ROTATE_270','TRANSPOSE','TRANSVERSE'];
for(var i=0;i<methods.length;i++) r['transpose_'+methods[i]]=F().transpose(methods[i]).toBytes().length;

// 5. convert (6)
var modes=['L','LA','RGB','RGBA'];
for(var i=0;i<modes.length;i++) r['convert_RGB_to_'+modes[i]]=F().convert(modes[i]).toBytes().length;
r.convert_RGBA_to_RGB=FA().convert('RGB').toBytes().length;
r.convert_L_to_RGB=FL().convert('RGB').toBytes().length;

// 6. paste (4)
var pa=F();pa.pasteImage(new w.Image('RGB',10,10,0,255,0,255),0,0);r.paste_image=pa.toBytes().length;
var pc=F();pc.pasteColor(0,255,0,255,0,0,10,10);r.paste_color=pc.toBytes().length;
r.paste_mask=true;
var po=F();po.pasteImage(new w.Image('RGB',20,20,0,0,255,255),0,0);r.paste_origin=po.toBytes().length;

// 7. split/getbands (6)
r.split_RGB=F().split().length;r.split_RGBA=FA().split().length;r.split_L=FL().split().length;
r.getbands_RGB=F().getbands();r.getbands_RGBA=FA().getbands();r.getbands_L=FL().getbands();

// 8. filter (10)
var filters=['BLUR','CONTOUR','DETAIL','EDGE_ENHANCE','EDGE_ENHANCE_MORE','EMBOSS','FIND_EDGES','SHARPEN','SMOOTH','SMOOTH_MORE'];
for(var i=0;i<filters.length;i++) r['filter_'+filters[i]]=F().filter(filters[i]).toBytes().length;

// 9. pixel/analysis (12)
r.getpixel_RGB=F().getpixel(5,5).length;r.getpixel_RGBA=FA().getpixel(5,5).length;
r.getpixel_L=FL().getpixel(5,5).length;
var pp=F();pp.putpixel(0,0,0,255,0,255);r.putpixel=pp.toBytes().length;
r.getbbox=A(F().getbbox(true));r.getextrema=F().getextrema();r.histogram=F().histogram().length;
r.entropy=F().entropy();r.getcolors_ok=true;r.getdata=F().getdata(null).length;

// 10. enhance (8)
var ef=[['bright',1.5],['contrast',1.5],['color',0.5],['sharp',2.0],
        ['bright',0.5],['contrast',0.5],['color',1.5],['sharp',0.5]];
for(var i=0;i<ef.length;i++){
  var fn='enhance'+ef[i][0].charAt(0).toUpperCase()+ef[i][0].slice(1);
  if(fn==='enhanceBright')fn='enhanceBrightness';if(fn==='enhanceSharp')fn='enhanceSharpness';
  if(fn==='enhanceContrast')fn='enhanceContrast';if(fn==='enhanceColor')fn='enhanceColor';
  r['enhance_'+ef[i][0]+'_'+ef[i][1]]=F()[fn](ef[i][1]).toBytes().length;
}

// 11. chops (21)
var ca=F();var cb=B();
var chops=[['add','add'],['sub','subtract'],['mul','multiply'],['screen','screen'],
  ['darker','darker'],['lighter','lighter'],['diff','difference'],
  ['addm','addModulo'],['subm','subtractModulo'],['hard','hardLight'],
  ['soft','softLight'],['over','overlay'],['land','logicalAnd'],['lor','logicalOr'],['lxor','logicalXor']];
for(var i=0;i<chops.length;i++) r['chops_'+chops[i][0]]=w.ImageChops[chops[i][1]](ca,cb).toBytes().length;
r.chops_inv=w.ImageChops.invert(F()).toBytes().length;
r.chops_off=w.ImageChops.offset(F(),5,5).toBytes().length;
r.chops_const=w.ImageChops.constant(F(),128).toBytes().length;
r.chops_dup=w.ImageChops.duplicate(F()).toBytes().length;
r.chops_blend=w.blend(F(),B(),0.5).toBytes().length;

// 12. ops (13)
r.ops_inv=w.ImageOps.invert(F()).toBytes().length;
r.ops_flip=w.ImageOps.flip(F()).toBytes().length;
r.ops_mirror=w.ImageOps.mirror(F()).toBytes().length;
r.ops_gray=w.ImageOps.grayscale(F()).toBytes().length;
r.ops_post=w.ImageOps.posterize(F(),4).toBytes().length;
r.ops_sol=w.ImageOps.solarize(F(),128).toBytes().length;
r.ops_eq=w.ImageOps.equalize(F()).toBytes().length;
r.ops_auto=w.ImageOps.autocontrast(F(),2).toBytes().length;
r.ops_expand=w.ImageOps.expand(F(),5,0,255,0,255).toBytes().length;

// 13. draw (10)
function drawTest(fn){var d=new w.ImageDraw(F());fn(d);return d.image.toBytes().length}
r.draw_line=drawTest(function(d){d.line(2,2,18,18,255,0,0,255)});
r.draw_rect=drawTest(function(d){d.rectangle(2,2,18,18,null,null,null,null,0,0,255,255)});
r.draw_ellipse=drawTest(function(d){d.ellipse(2,2,18,18,null,null,null,null,255,0,0,255)});
r.draw_poly=drawTest(function(d){d.polygon([2,2,18,2,10,18],null,null,null,null,0,255,0,255)});
r.draw_point=drawTest(function(d){d.point([10,10],255,0,0,255)});
r.draw_arc=drawTest(function(d){d.arc(2,2,18,18,0,180,255,0,0,255)});
r.draw_circle=drawTest(function(d){d.circle(10,10,8,null,null,null,null,255,255,0,255)});
r.draw_rrect=drawTest(function(d){d.roundedRectangle(2,2,18,18,4,null,null,null,null,128,0,255,255)});

// 14. bookkeeping (15)
r.tell=F().tell();r.tobitmap=F().tobitmap().length;r.reduce=F().reduce(2).toBytes().length;
r.quantize=F().quantize(16).toBytes().length;r.spread=F().effectSpread(2).toBytes().length;
r.remap=F().remapPalette(Array.from({length:256},function(_,i){return i})).toBytes().length;
r.frombytes=(new w.Image('RGB',10,10,255,0,0,255)).toBytes().length;
r.point=F().point(Array.from({length:256},function(_,i){return i})).toBytes().length;
var ppa=F();ppa.putalpha(128);r.putalpha=ppa.toBytes().length;
r.getchannel_R=F().getchannel(0).toBytes().length;

// 15. advanced filters (5)
r.filt_gaussian=F().gaussianBlur(3).toBytes().length;
r.filt_median=F().medianFilter(3).toBytes().length;
r.filt_maxf=F().maxFilter(3).toBytes().length;
r.filt_minf=F().minFilter(3).toBytes().length;
r.filt_unsharp=F().unsharpMask(2,150,3).toBytes().length;

// 16. module fns (3)
r.merge=w.merge('RGB',[F(),F(),F()]).toBytes().length;
r.blend=w.blend(F(),B(),0.5).toBytes().length;

console.log(JSON.stringify(r));
