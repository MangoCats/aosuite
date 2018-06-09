#ifndef __AESECBVARTXT256__
#define __AESECBVARTXT256__

#include "../plainkeycipher.h"

// Test vectors from <http://csrc.nist.gov/groups/STM/cavp/documents/aes/KAT_AES.zip>

const std::vector <PlainKeyCipher> AES256_VARTXT = {
    std::make_tuple("80000000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "ddc6bf790c15760d8d9aeb6f9a75fd4e"),
    std::make_tuple("c0000000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "0a6bdc6d4c1e6280301fd8e97ddbe601"),
    std::make_tuple("e0000000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "9b80eefb7ebe2d2b16247aa0efc72f5d"),
    std::make_tuple("f0000000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "7f2c5ece07a98d8bee13c51177395ff7"),
    std::make_tuple("f8000000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "7818d800dcf6f4be1e0e94f403d1e4c2"),
    std::make_tuple("fc000000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "e74cd1c92f0919c35a0324123d6177d3"),
    std::make_tuple("fe000000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "8092a4dcf2da7e77e93bdd371dfed82e"),
    std::make_tuple("ff000000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "49af6b372135acef10132e548f217b17"),
    std::make_tuple("ff800000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "8bcd40f94ebb63b9f7909676e667f1e7"),
    std::make_tuple("ffc00000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "fe1cffb83f45dcfb38b29be438dbd3ab"),
    std::make_tuple("ffe00000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "0dc58a8d886623705aec15cb1e70dc0e"),
    std::make_tuple("fff00000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "c218faa16056bd0774c3e8d79c35a5e4"),
    std::make_tuple("fff80000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "047bba83f7aa841731504e012208fc9e"),
    std::make_tuple("fffc0000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "dc8f0e4915fd81ba70a331310882f6da"),
    std::make_tuple("fffe0000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "1569859ea6b7206c30bf4fd0cbfac33c"),
    std::make_tuple("ffff0000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "300ade92f88f48fa2df730ec16ef44cd"),
    std::make_tuple("ffff8000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "1fe6cc3c05965dc08eb0590c95ac71d0"),
    std::make_tuple("ffffc000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "59e858eaaa97fec38111275b6cf5abc0"),
    std::make_tuple("ffffe000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "2239455e7afe3b0616100288cc5a723b"),
    std::make_tuple("fffff000000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "3ee500c5c8d63479717163e55c5c4522"),
    std::make_tuple("fffff800000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "d5e38bf15f16d90e3e214041d774daa8"),
    std::make_tuple("fffffc00000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "b1f4066e6f4f187dfe5f2ad1b17819d0"),
    std::make_tuple("fffffe00000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "6ef4cc4de49b11065d7af2909854794a"),
    std::make_tuple("ffffff00000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "ac86bc606b6640c309e782f232bf367f"),
    std::make_tuple("ffffff80000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "36aff0ef7bf3280772cf4cac80a0d2b2"),
    std::make_tuple("ffffffc0000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "1f8eedea0f62a1406d58cfc3ecea72cf"),
    std::make_tuple("ffffffe0000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "abf4154a3375a1d3e6b1d454438f95a6"),
    std::make_tuple("fffffff0000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "96f96e9d607f6615fc192061ee648b07"),
    std::make_tuple("fffffff8000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "cf37cdaaa0d2d536c71857634c792064"),
    std::make_tuple("fffffffc000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "fbd6640c80245c2b805373f130703127"),
    std::make_tuple("fffffffe000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "8d6a8afe55a6e481badae0d146f436db"),
    std::make_tuple("ffffffff000000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "6a4981f2915e3e68af6c22385dd06756"),
    std::make_tuple("ffffffff800000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "42a1136e5f8d8d21d3101998642d573b"),
    std::make_tuple("ffffffffc00000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "9b471596dc69ae1586cee6158b0b0181"),
    std::make_tuple("ffffffffe00000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "753665c4af1eff33aa8b628bf8741cfd"),
    std::make_tuple("fffffffff00000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "9a682acf40be01f5b2a4193c9a82404d"),
    std::make_tuple("fffffffff80000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "54fafe26e4287f17d1935f87eb9ade01"),
    std::make_tuple("fffffffffc0000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "49d541b2e74cfe73e6a8e8225f7bd449"),
    std::make_tuple("fffffffffe0000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "11a45530f624ff6f76a1b3826626ff7b"),
    std::make_tuple("ffffffffff0000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "f96b0c4a8bc6c86130289f60b43b8fba"),
    std::make_tuple("ffffffffff8000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "48c7d0e80834ebdc35b6735f76b46c8b"),
    std::make_tuple("ffffffffffc000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "2463531ab54d66955e73edc4cb8eaa45"),
    std::make_tuple("ffffffffffe000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "ac9bd8e2530469134b9d5b065d4f565b"),
    std::make_tuple("fffffffffff000000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "3f5f9106d0e52f973d4890e6f37e8a00"),
    std::make_tuple("fffffffffff800000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "20ebc86f1304d272e2e207e59db639f0"),
    std::make_tuple("fffffffffffc00000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "e67ae6426bf9526c972cff072b52252c"),
    std::make_tuple("fffffffffffe00000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "1a518dddaf9efa0d002cc58d107edfc8"),
    std::make_tuple("ffffffffffff00000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "ead731af4d3a2fe3b34bed047942a49f"),
    std::make_tuple("ffffffffffff80000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "b1d4efe40242f83e93b6c8d7efb5eae9"),
    std::make_tuple("ffffffffffffc0000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "cd2b1fec11fd906c5c7630099443610a"),
    std::make_tuple("ffffffffffffe0000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "a1853fe47fe29289d153161d06387d21"),
    std::make_tuple("fffffffffffff0000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "4632154179a555c17ea604d0889fab14"),
    std::make_tuple("fffffffffffff8000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "dd27cac6401a022e8f38f9f93e774417"),
    std::make_tuple("fffffffffffffc000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "c090313eb98674f35f3123385fb95d4d"),
    std::make_tuple("fffffffffffffe000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "cc3526262b92f02edce548f716b9f45c"),
    std::make_tuple("ffffffffffffff000000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "c0838d1a2b16a7c7f0dfcc433c399c33"),
    std::make_tuple("ffffffffffffff800000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "0d9ac756eb297695eed4d382eb126d26"),
    std::make_tuple("ffffffffffffffc00000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "56ede9dda3f6f141bff1757fa689c3e1"),
    std::make_tuple("ffffffffffffffe00000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "768f520efe0f23e61d3ec8ad9ce91774"),
    std::make_tuple("fffffffffffffff00000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "b1144ddfa75755213390e7c596660490"),
    std::make_tuple("fffffffffffffff80000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "1d7c0c4040b355b9d107a99325e3b050"),
    std::make_tuple("fffffffffffffffc0000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "d8e2bb1ae8ee3dcf5bf7d6c38da82a1a"),
    std::make_tuple("fffffffffffffffe0000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "faf82d178af25a9886a47e7f789b98d7"),
    std::make_tuple("ffffffffffffffff0000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "9b58dbfd77fe5aca9cfc190cd1b82d19"),
    std::make_tuple("ffffffffffffffff8000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "77f392089042e478ac16c0c86a0b5db5"),
    std::make_tuple("ffffffffffffffffc000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "19f08e3420ee69b477ca1420281c4782"),
    std::make_tuple("ffffffffffffffffe000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "a1b19beee4e117139f74b3c53fdcb875"),
    std::make_tuple("fffffffffffffffff000000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "a37a5869b218a9f3a0868d19aea0ad6a"),
    std::make_tuple("fffffffffffffffff800000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "bc3594e865bcd0261b13202731f33580"),
    std::make_tuple("fffffffffffffffffc00000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "811441ce1d309eee7185e8c752c07557"),
    std::make_tuple("fffffffffffffffffe00000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "959971ce4134190563518e700b9874d1"),
    std::make_tuple("ffffffffffffffffff00000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "76b5614a042707c98e2132e2e805fe63"),
    std::make_tuple("ffffffffffffffffff80000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "7d9fa6a57530d0f036fec31c230b0cc6"),
    std::make_tuple("ffffffffffffffffffc0000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "964153a83bf6989a4ba80daa91c3e081"),
    std::make_tuple("ffffffffffffffffffe0000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "a013014d4ce8054cf2591d06f6f2f176"),
    std::make_tuple("fffffffffffffffffff0000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "d1c5f6399bf382502e385eee1474a869"),
    std::make_tuple("fffffffffffffffffff8000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "0007e20b8298ec354f0f5fe7470f36bd"),
    std::make_tuple("fffffffffffffffffffc000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "b95ba05b332da61ef63a2b31fcad9879"),
    std::make_tuple("fffffffffffffffffffe000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "4620a49bd967491561669ab25dce45f4"),
    std::make_tuple("ffffffffffffffffffff000000000000", "0000000000000000000000000000000000000000000000000000000000000000", "12e71214ae8e04f0bb63d7425c6f14d5"),
    std::make_tuple("ffffffffffffffffffff800000000000", "0000000000000000000000000000000000000000000000000000000000000000", "4cc42fc1407b008fe350907c092e80ac"),
    std::make_tuple("ffffffffffffffffffffc00000000000", "0000000000000000000000000000000000000000000000000000000000000000", "08b244ce7cbc8ee97fbba808cb146fda"),
    std::make_tuple("ffffffffffffffffffffe00000000000", "0000000000000000000000000000000000000000000000000000000000000000", "39b333e8694f21546ad1edd9d87ed95b"),
    std::make_tuple("fffffffffffffffffffff00000000000", "0000000000000000000000000000000000000000000000000000000000000000", "3b271f8ab2e6e4a20ba8090f43ba78f3"),
    std::make_tuple("fffffffffffffffffffff80000000000", "0000000000000000000000000000000000000000000000000000000000000000", "9ad983f3bf651cd0393f0a73cccdea50"),
    std::make_tuple("fffffffffffffffffffffc0000000000", "0000000000000000000000000000000000000000000000000000000000000000", "8f476cbff75c1f725ce18e4bbcd19b32"),
    std::make_tuple("fffffffffffffffffffffe0000000000", "0000000000000000000000000000000000000000000000000000000000000000", "905b6267f1d6ab5320835a133f096f2a"),
    std::make_tuple("ffffffffffffffffffffff0000000000", "0000000000000000000000000000000000000000000000000000000000000000", "145b60d6d0193c23f4221848a892d61a"),
    std::make_tuple("ffffffffffffffffffffff8000000000", "0000000000000000000000000000000000000000000000000000000000000000", "55cfb3fb6d75cad0445bbc8dafa25b0f"),
    std::make_tuple("ffffffffffffffffffffffc000000000", "0000000000000000000000000000000000000000000000000000000000000000", "7b8e7098e357ef71237d46d8b075b0f5"),
    std::make_tuple("ffffffffffffffffffffffe000000000", "0000000000000000000000000000000000000000000000000000000000000000", "2bf27229901eb40f2df9d8398d1505ae"),
    std::make_tuple("fffffffffffffffffffffff000000000", "0000000000000000000000000000000000000000000000000000000000000000", "83a63402a77f9ad5c1e931a931ecd706"),
    std::make_tuple("fffffffffffffffffffffff800000000", "0000000000000000000000000000000000000000000000000000000000000000", "6f8ba6521152d31f2bada1843e26b973"),
    std::make_tuple("fffffffffffffffffffffffc00000000", "0000000000000000000000000000000000000000000000000000000000000000", "e5c3b8e30fd2d8e6239b17b44bd23bbd"),
    std::make_tuple("fffffffffffffffffffffffe00000000", "0000000000000000000000000000000000000000000000000000000000000000", "1ac1f7102c59933e8b2ddc3f14e94baa"),
    std::make_tuple("ffffffffffffffffffffffff00000000", "0000000000000000000000000000000000000000000000000000000000000000", "21d9ba49f276b45f11af8fc71a088e3d"),
    std::make_tuple("ffffffffffffffffffffffff80000000", "0000000000000000000000000000000000000000000000000000000000000000", "649f1cddc3792b4638635a392bc9bade"),
    std::make_tuple("ffffffffffffffffffffffffc0000000", "0000000000000000000000000000000000000000000000000000000000000000", "e2775e4b59c1bc2e31a2078c11b5a08c"),
    std::make_tuple("ffffffffffffffffffffffffe0000000", "0000000000000000000000000000000000000000000000000000000000000000", "2be1fae5048a25582a679ca10905eb80"),
    std::make_tuple("fffffffffffffffffffffffff0000000", "0000000000000000000000000000000000000000000000000000000000000000", "da86f292c6f41ea34fb2068df75ecc29"),
    std::make_tuple("fffffffffffffffffffffffff8000000", "0000000000000000000000000000000000000000000000000000000000000000", "220df19f85d69b1b562fa69a3c5beca5"),
    std::make_tuple("fffffffffffffffffffffffffc000000", "0000000000000000000000000000000000000000000000000000000000000000", "1f11d5d0355e0b556ccdb6c7f5083b4d"),
    std::make_tuple("fffffffffffffffffffffffffe000000", "0000000000000000000000000000000000000000000000000000000000000000", "62526b78be79cb384633c91f83b4151b"),
    std::make_tuple("ffffffffffffffffffffffffff000000", "0000000000000000000000000000000000000000000000000000000000000000", "90ddbcb950843592dd47bbef00fdc876"),
    std::make_tuple("ffffffffffffffffffffffffff800000", "0000000000000000000000000000000000000000000000000000000000000000", "2fd0e41c5b8402277354a7391d2618e2"),
    std::make_tuple("ffffffffffffffffffffffffffc00000", "0000000000000000000000000000000000000000000000000000000000000000", "3cdf13e72dee4c581bafec70b85f9660"),
    std::make_tuple("ffffffffffffffffffffffffffe00000", "0000000000000000000000000000000000000000000000000000000000000000", "afa2ffc137577092e2b654fa199d2c43"),
    std::make_tuple("fffffffffffffffffffffffffff00000", "0000000000000000000000000000000000000000000000000000000000000000", "8d683ee63e60d208e343ce48dbc44cac"),
    std::make_tuple("fffffffffffffffffffffffffff80000", "0000000000000000000000000000000000000000000000000000000000000000", "705a4ef8ba2133729c20185c3d3a4763"),
    std::make_tuple("fffffffffffffffffffffffffffc0000", "0000000000000000000000000000000000000000000000000000000000000000", "0861a861c3db4e94194211b77ed761b9"),
    std::make_tuple("fffffffffffffffffffffffffffe0000", "0000000000000000000000000000000000000000000000000000000000000000", "4b00c27e8b26da7eab9d3a88dec8b031"),
    std::make_tuple("ffffffffffffffffffffffffffff0000", "0000000000000000000000000000000000000000000000000000000000000000", "5f397bf03084820cc8810d52e5b666e9"),
    std::make_tuple("ffffffffffffffffffffffffffff8000", "0000000000000000000000000000000000000000000000000000000000000000", "63fafabb72c07bfbd3ddc9b1203104b8"),
    std::make_tuple("ffffffffffffffffffffffffffffc000", "0000000000000000000000000000000000000000000000000000000000000000", "683e2140585b18452dd4ffbb93c95df9"),
    std::make_tuple("ffffffffffffffffffffffffffffe000", "0000000000000000000000000000000000000000000000000000000000000000", "286894e48e537f8763b56707d7d155c8"),
    std::make_tuple("fffffffffffffffffffffffffffff000", "0000000000000000000000000000000000000000000000000000000000000000", "a423deabc173dcf7e2c4c53e77d37cd1"),
    std::make_tuple("fffffffffffffffffffffffffffff800", "0000000000000000000000000000000000000000000000000000000000000000", "eb8168313e1cfdfdb5e986d5429cf172"),
    std::make_tuple("fffffffffffffffffffffffffffffc00", "0000000000000000000000000000000000000000000000000000000000000000", "27127daafc9accd2fb334ec3eba52323"),
    std::make_tuple("fffffffffffffffffffffffffffffe00", "0000000000000000000000000000000000000000000000000000000000000000", "ee0715b96f72e3f7a22a5064fc592f4c"),
    std::make_tuple("ffffffffffffffffffffffffffffff00", "0000000000000000000000000000000000000000000000000000000000000000", "29ee526770f2a11dcfa989d1ce88830f"),
    std::make_tuple("ffffffffffffffffffffffffffffff80", "0000000000000000000000000000000000000000000000000000000000000000", "0493370e054b09871130fe49af730a5a"),
    std::make_tuple("ffffffffffffffffffffffffffffffc0", "0000000000000000000000000000000000000000000000000000000000000000", "9b7b940f6c509f9e44a4ee140448ee46"),
    std::make_tuple("ffffffffffffffffffffffffffffffe0", "0000000000000000000000000000000000000000000000000000000000000000", "2915be4a1ecfdcbe3e023811a12bb6c7"),
    std::make_tuple("fffffffffffffffffffffffffffffff0", "0000000000000000000000000000000000000000000000000000000000000000", "7240e524bc51d8c4d440b1be55d1062c"),
    std::make_tuple("fffffffffffffffffffffffffffffff8", "0000000000000000000000000000000000000000000000000000000000000000", "da63039d38cb4612b2dc36ba26684b93"),
    std::make_tuple("fffffffffffffffffffffffffffffffc", "0000000000000000000000000000000000000000000000000000000000000000", "0f59cb5a4b522e2ac56c1a64f558ad9a"),
    std::make_tuple("fffffffffffffffffffffffffffffffe", "0000000000000000000000000000000000000000000000000000000000000000", "7bfe9d876c6d63c1d035da8fe21c409d"),
    std::make_tuple("ffffffffffffffffffffffffffffffff", "0000000000000000000000000000000000000000000000000000000000000000", "acdace8078a32b1a182bfa4987ca1347"),
};

#endif // __AESECBVARTXT256__
