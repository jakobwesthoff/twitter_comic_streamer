# Training process

The following file describes how the `comic_net` was created in the first place. This should ease the creation of the net with custom training data.

## Preparation of training material

In order to retrieve enough training material to classify comics, my [twitter_image_downloader](https://github.com/jakobwesthoff/twitter_image_downloader) was used to retrieve all currently available images from the following twitter accounts:

* daskritzelt
* erzaehlmirnix
* islieb
* isfies666
* joschasauer
* ralphruthe
* foxes_in_love
* hauckundbauer
* dino_comics

```shell
for i in daskritzelt erzaehlmirnix islieb isfies666 joschasauer ralphruthe foxes_in_love hauckundbauer dino_comics; do \
  twitter_image_downloader \
    --consumer-key $CONSUMER_KEY \
    --consumer-secret $CONSUMER_SECRET \
    --access-token $ACCESS_TOKEN \
    --access-token-secret $ACCESS_TOKEN_SECRET \
    -o ./download_$i \
    -u urls_$i.log \
    -m 16 \
    $i; \
done
```

After downloading the images they were manually classified into two different folders: `./images/comic` and `./images/no_comic`. The result is a training set of 6340 comic pictures and 778 no_comic pictures.

### Retrieving the same data set

Due to copyright as well as storage amount reasons the data set can not be part of this repository. However there is an alternative way to retrieve the same data set for training. The manually classified images have been stored as URL lists and are available in the files `urls_comic.log` and `urls_no_comic.log`. Furthermore the script `download_images.sh` downloads and prepares the images in the correct folder structure.

## Retraining the Model

Once the images are in place the [make_image_classifier](https://github.com/tensorflow/hub/tree/c27a78e953a39fc6928233f3ef3da1d7121a0baf/tensorflow_hub/tools/make_image_classifier) tool of the TensorFlow/hub can be utilized to retrain the Mobile-Net model to classify the comics correctly.

```shell
make_image_classifier \
  --tfhub_module https://tfhub.dev/google/imagenet/mobilenet_v2_100_128/feature_vector/5 \
  --tfhub_cache_dir=./hub_cache/ \
  --image_dir=./images/ \
  --summaries_dir ./comic_net_log/ \
  --saved_model_dir=./comic_net/ \
  --tflite_output_file comic_net.tflite \
  --labels_output_file comic_net.labels \
  --train_epochs=12 \
  --image_size=128 \
  --batch_size=32 \
  --do_fine_tuning=true
```

*Note:* You might want to grab out a couple (30 or so) pictures from each class before training, remove them from the training set and later on use them to test the trained net.

```shell
mkdir -p test/{comic,no_comic}
ls images/comic|sort -R |tail -30 |while read -r file; do mv -v images/comic/$file test/comic/$file; done
ls images/no_comic|sort -R |tail -30 |while read -r file; do mv -v images/no_comic/$file test/no_comic/$file; done
```

In order to label the images from this test set, use the `label_image.py`, which was taken from the TensorFlow repository:

```shell
python label_image.py -m comic_net.tflite -l comic_net.lables -i ./test/no_comic --check_label 0
python label_image.py -m comic_net.tflite -l comic_net.lables -i ./test/comic --check_label 1
```

The commands above will try and classify the given images and output a list of
misclassified ones.

## Help wanted

My knowledge with regards to neural nets as well as TensorFlow is very limited. I have a basic understanding of how deep neural nets work and what they do. I can use existing nets, or use the proper tooling as done here to retrain existing nets. However I am lacking the knowledge to optimize, or maybe even create a completely new neural net to do such a classification. Even though my retrained net works well enough for my use case, I would love to have a more precise one. If you have a deeper understanding of those ML techniques and want to help out, please get in touch.
