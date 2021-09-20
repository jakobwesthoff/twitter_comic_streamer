# Comic Image Classification

## Usage

The provided python script (`app.py`) provides a web server on port 5000 which takes images as POST body against the `/classify` route. The response is a JSON object containing the matched label as well as the probability of the match.

## Prerequisites

The Script needs a TensorFlow 2.x lite installation ready to be used with python as well as the modules specified within the `requirements.txt`.

## Used neural net

The `comic_net` is a TensorFlow based neural net for image detection. It is based on the [Mobile-Net v2](https://tfhub.dev/google/imagenet/mobilenet_v2_100_128/feature_vector/5) and retrained to classify into the two labels `comic` and `no_comic`. Details about how the model was trained and against what kind of training data can be found within the `training` folder.
